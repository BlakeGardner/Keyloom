//! systemd's user manager, over its D-Bus API on the session bus.
//!
//! Everything [`crate::service`] asks of systemd goes through here: a
//! unit's state, starting, stopping, and restarting it, enabling it,
//! reloading unit files, and the signals that announce changes. These
//! are the requests `systemctl --user` sends, made directly, so no
//! `systemctl` has to be installed (a sandbox has none) and nothing is
//! read back from a command's output.
//!
//! Starting, stopping, or restarting a unit only queues a job, and the
//! request returns before the job has run. Like `systemctl`,
//! [`Manager::start`] and its siblings wait for systemd to report the
//! job removed, and succeed only when it finished as asked. A finished
//! job says nothing about what happens next: a service that dies right
//! after starting shows up as a later change of state, which
//! [`Manager::changes`] announces.

use std::collections::HashMap;
use std::fmt::{self, Write as _};
use std::io;
use std::sync::Arc;

use cosmic::iced::futures::StreamExt;
use cosmic::iced::futures::future::ready;
use cosmic::iced::futures::stream::{self, BoxStream, SelectAll};
use zbus::message::{Sequence, Type};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, MatchRule, MessageStream};

/// The bus name systemd's manager answers to.
const SYSTEMD: &str = "org.freedesktop.systemd1";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER_INTERFACE: &str = "org.freedesktop.systemd1.Manager";
const UNIT_INTERFACE: &str = "org.freedesktop.systemd1.Unit";
const SERVICE_INTERFACE: &str = "org.freedesktop.systemd1.Service";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

/// Where the units' objects live; a unit's own path adds its escaped
/// name ([`object_path`]).
const UNIT_PATH_PREFIX: &str = "/org/freedesktop/systemd1/unit/";

/// D-Bus errors meaning that nobody on the bus answers for systemd, as
/// opposed to systemd answering no.
const NO_MANAGER: [&str; 4] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
    "org.freedesktop.DBus.Error.NoReply",
    "org.freedesktop.DBus.Error.Disconnected",
];

/// What systemd answers a second `Subscribe` from the same connection.
const ALREADY_SUBSCRIBED: &str = "org.freedesktop.systemd1.AlreadySubscribed";

/// Why systemd did not do what Keyloom asked.
///
/// Held in a [`crate::app::Message`], which must be `Clone`, so the
/// underlying bus error is shared rather than copied.
#[derive(Clone, Debug)]
pub enum Error {
    /// There is no session bus, or no systemd user manager on it, or
    /// what came back could not be understood.
    Unavailable(Arc<zbus::Error>),
    /// systemd refused, or the job did not finish as asked; the text is
    /// systemd's explanation.
    Refused(String),
}

impl fmt::Display for Error {
    /// Written to compose after a prefix, as error messages do:
    /// lowercase, no trailing punctuation.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(err) => {
                write!(f, "could not talk to systemd's user manager: {err}")
            }
            Self::Refused(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(err) => Some(&**err),
            Self::Refused(_) => None,
        }
    }
}

impl From<zbus::Error> for Error {
    /// systemd's own error replies carry its explanation; anything else
    /// means systemd could not be asked.
    fn from(err: zbus::Error) -> Self {
        if let zbus::Error::MethodError(name, description, _) = &err
            && !NO_MANAGER.contains(&name.as_str())
        {
            return Self::Refused(clause(description.as_deref().unwrap_or(name.as_str())));
        }
        Self::Unavailable(Arc::new(err))
    }
}

/// A message of systemd's as a clause that follows a prefix: without
/// the trailing period, and lowercase unless it starts with an acronym
/// (`Unit x not found.` becomes `unit x not found`).
fn clause(message: &str) -> String {
    let message = message.trim().trim_end_matches('.');
    let mut chars = message.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_uppercase() && second.is_lowercase() => first
            .to_lowercase()
            .chain(message[first.len_utf8()..].chars())
            .collect(),
        _ => message.to_owned(),
    }
}

/// The object path systemd serves a unit at: the unit's name with every
/// byte that is not an ASCII letter, or a digit after the first byte,
/// spelled as `_` and two lowercase hex digits (systemd's
/// `bus_label_escape`).
pub fn object_path(name: &str) -> String {
    let mut path = String::from(UNIT_PATH_PREFIX);
    if name.is_empty() {
        path.push('_');
    }
    for (index, byte) in name.bytes().enumerate() {
        if byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit()) {
            path.push(char::from(byte));
        } else {
            // Writing to a String cannot fail.
            let _ = write!(path, "_{byte:02x}");
        }
    }
    path
}

/// What systemd reports about a unit: the properties Keyloom reads, in
/// systemd's own words. A property systemd did not send is empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitState {
    /// `loaded`, `not-found`, `masked`, …
    pub load_state: String,
    /// `active`, `activating`, `deactivating`, `failed`, …
    pub active_state: String,
    /// The finer state the unit's type knows: `running`, `start-pre`,
    /// `auto-restart`, …
    pub sub_state: String,
    /// `enabled`, `disabled`, `static`, …
    pub unit_file_state: String,
    /// The unit file systemd loaded, or nothing.
    pub fragment_path: String,
}

impl UnitState {
    /// Pick the properties out of the unit interface's `GetAll` reply.
    fn from_properties(properties: &HashMap<String, OwnedValue>) -> Self {
        let text = |name: &str| {
            properties
                .get(name)
                .and_then(|value| <&str>::try_from(value).ok())
                .unwrap_or_default()
                .to_owned()
        };
        Self {
            load_state: text("LoadState"),
            active_state: text("ActiveState"),
            sub_state: text("SubState"),
            unit_file_state: text("UnitFileState"),
            fragment_path: text("FragmentPath"),
        }
    }
}

/// Something that may have changed what systemd reports about a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// The unit's state changed, or systemd reloaded its unit files.
    Unit,
    /// systemd's user manager left the bus or took its name again, as a
    /// re-exec does; a manager that comes back may need a new
    /// [`Manager::subscribe`].
    Manager,
}

/// The stream [`Manager::changes`] returns.
pub type Changes = SelectAll<BoxStream<'static, Change>>;

/// A connection to systemd's user manager. Cloning shares the
/// connection.
#[derive(Clone, Debug)]
pub struct Manager {
    bus: Connection,
}

impl Manager {
    /// Connect to the session bus, where the user manager is.
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] when there is no session bus to connect
    /// to.
    pub async fn session() -> Result<Self, Error> {
        Ok(Self {
            bus: Connection::session().await?,
        })
    }

    /// What systemd reports about a unit, and where on this connection
    /// its reply was received. systemd replies in the order it handles
    /// requests, so of two replies, the one received later describes
    /// the later moment. Asking about a unit systemd has not loaded
    /// loads it, which is how a missing unit reads as `not-found`.
    ///
    /// # Errors
    ///
    /// When systemd cannot be asked, or refuses to say.
    pub async fn unit(&self, name: &str) -> Result<(UnitState, Sequence), Error> {
        let path = object_path(name);
        let reply = self
            .bus
            .call_method(
                Some(SYSTEMD),
                path.as_str(),
                Some(PROPERTIES_INTERFACE),
                "GetAll",
                &UNIT_INTERFACE,
            )
            .await?;
        let properties: HashMap<String, OwnedValue> = reply.body().deserialize()?;
        Ok((
            UnitState::from_properties(&properties),
            reply.recv_position(),
        ))
    }

    /// Start a unit, as `systemctl --user start` does. Starting a unit
    /// that already runs succeeds without doing anything.
    ///
    /// # Errors
    ///
    /// When systemd cannot be asked, refuses (no such unit, a masked
    /// one), or the job fails (a failing `ExecStartPre`, the start
    /// rate limit).
    pub async fn start(&self, name: &str) -> Result<(), Error> {
        self.job("StartUnit", name).await
    }

    /// Stop a unit, as `systemctl --user stop` does. Stopping a unit
    /// that does not run succeeds without doing anything.
    ///
    /// # Errors
    ///
    /// As [`Self::start`].
    pub async fn stop(&self, name: &str) -> Result<(), Error> {
        self.job("StopUnit", name).await
    }

    /// Restart a unit, as `systemctl --user restart` does, starting it
    /// when it does not run.
    ///
    /// # Errors
    ///
    /// As [`Self::start`].
    pub async fn restart(&self, name: &str) -> Result<(), Error> {
        self.job("RestartUnit", name).await
    }

    /// Queue a job with one of the manager's `…Unit` methods and wait
    /// for systemd to report it removed.
    async fn job(&self, method: &'static str, name: &str) -> Result<(), Error> {
        // Listen first: once queued, the job can finish before the
        // reply naming it has been read. Every removed job is heard and
        // the path picks this one out, as `systemctl` does: matching on
        // the unit's name, which follows a number and a path in the
        // signal, is something dbus-broker cannot do.
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(SYSTEMD)?
            .path(MANAGER_PATH)?
            .interface(MANAGER_INTERFACE)?
            .member("JobRemoved")?
            .build();
        let mut removed = MessageStream::for_match_rule(rule, &self.bus, None).await?;
        // "replace" is `systemctl`'s own mode: the request supersedes
        // a conflicting job already queued for the unit.
        let reply = self
            .bus
            .call_method(
                Some(SYSTEMD),
                MANAGER_PATH,
                Some(MANAGER_INTERFACE),
                method,
                &(name, "replace"),
            )
            .await?;
        let job: OwnedObjectPath = reply.body().deserialize()?;
        while let Some(signal) = removed.next().await {
            let (_id, removed_job, _unit, result): (u32, OwnedObjectPath, String, String) =
                signal?.body().deserialize()?;
            if removed_job != job {
                continue;
            }
            if job_succeeded(&result) {
                return Ok(());
            }
            let service_result = self.service_result(name).await;
            return Err(Error::Refused(job_failure(
                name,
                &result,
                service_result.as_deref(),
            )));
        }
        Err(Error::Unavailable(Arc::new(zbus::Error::InputOutput(
            Arc::new(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "the session bus closed before the job finished",
            )),
        ))))
    }

    /// Why a service's last run ended (its `Result` property:
    /// `exit-code`, `start-limit-hit`, …), when systemd says.
    async fn service_result(&self, name: &str) -> Option<String> {
        let path = object_path(name);
        let reply = self
            .bus
            .call_method(
                Some(SYSTEMD),
                path.as_str(),
                Some(PROPERTIES_INTERFACE),
                "Get",
                &(SERVICE_INTERFACE, "Result"),
            )
            .await
            .ok()?;
        let value: OwnedValue = reply.body().deserialize().ok()?;
        value.downcast_ref::<&str>().ok().map(str::to_owned)
    }

    /// Start a unit with every login from now on, as
    /// `systemctl --user enable` does, including the reload that
    /// follows.
    ///
    /// # Errors
    ///
    /// When systemd cannot be asked, or refuses (no such unit file, a
    /// conflicting link already in place).
    pub async fn enable(&self, name: &str) -> Result<(), Error> {
        // Not only for this session (`runtime`), and without replacing
        // links that point elsewhere (`force`), as `systemctl` defaults.
        self.bus
            .call_method(
                Some(SYSTEMD),
                MANAGER_PATH,
                Some(MANAGER_INTERFACE),
                "EnableUnitFiles",
                &(&[name][..], false, false),
            )
            .await?;
        self.reload().await
    }

    /// Have systemd read its unit files again, as
    /// `systemctl --user daemon-reload` does. systemd replies once the
    /// reload is complete.
    ///
    /// # Errors
    ///
    /// When systemd cannot be asked, or refuses.
    pub async fn reload(&self) -> Result<(), Error> {
        self.call("Reload").await
    }

    /// Ask systemd to announce changes on the bus, which it does only
    /// while a client is subscribed. The subscription lasts as long as
    /// this connection.
    ///
    /// # Errors
    ///
    /// When systemd cannot be asked, or refuses.
    pub async fn subscribe(&self) -> Result<(), Error> {
        let reply = self
            .bus
            .call_method(
                Some(SYSTEMD),
                MANAGER_PATH,
                Some(MANAGER_INTERFACE),
                "Subscribe",
                &(),
            )
            .await;
        match reply {
            Ok(_) => Ok(()),
            // A manager that re-executed keeps its subscribers, and
            // says so when asked again.
            Err(zbus::Error::MethodError(name, ..)) if name.as_str() == ALREADY_SUBSCRIBED => {
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Call one of the manager's methods that take and return nothing.
    async fn call(&self, method: &'static str) -> Result<(), Error> {
        self.bus
            .call_method(
                Some(SYSTEMD),
                MANAGER_PATH,
                Some(MANAGER_INTERFACE),
                method,
                &(),
            )
            .await?;
        Ok(())
    }

    /// Every event after which what systemd reports about a unit may
    /// read differently: the unit's own properties changing, systemd
    /// reloading its unit files, and systemd's manager coming or going.
    /// The unit's properties change only while someone is subscribed
    /// ([`Self::subscribe`]).
    ///
    /// Loading and unloading the unit is left out on purpose: asking
    /// about an unloaded unit loads it and systemd unloads it again
    /// right away, so answering those with another look would never
    /// end. Neither changes the unit's state.
    ///
    /// The stream ends when the connection closes.
    ///
    /// # Errors
    ///
    /// When the bus refuses to route the signals.
    pub async fn changes(&self, name: &str) -> Result<Changes, Error> {
        let path = object_path(name);
        // Only the unit interface: every change of state also comes
        // with one for the service interface, which says nothing more.
        let properties = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(SYSTEMD)?
            .path(path.as_str())?
            .interface(PROPERTIES_INTERFACE)?
            .member("PropertiesChanged")?
            .arg(0, UNIT_INTERFACE)?
            .build();
        let reloading = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(SYSTEMD)?
            .path(MANAGER_PATH)?
            .interface(MANAGER_INTERFACE)?
            .member("Reloading")?
            .build();
        let owner = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender("org.freedesktop.DBus")?
            .path("/org/freedesktop/DBus")?
            .interface("org.freedesktop.DBus")?
            .member("NameOwnerChanged")?
            .arg(0, SYSTEMD)?
            .build();
        Ok(stream::select_all([
            self.announcing(properties, Change::Unit).await?,
            self.announcing(reloading, Change::Unit).await?,
            self.announcing(owner, Change::Manager).await?,
        ]))
    }

    /// The signals a rule matches, each announced as `change`.
    async fn announcing(
        &self,
        rule: MatchRule<'_>,
        change: Change,
    ) -> Result<BoxStream<'static, Change>, Error> {
        let signals = MessageStream::for_match_rule(rule, &self.bus, None).await?;
        Ok(signals
            .filter_map(move |signal| ready(signal.ok().map(|_| change)))
            .boxed())
    }
}

/// Whether a removed job finished as asked. `skipped` is a start that
/// had nothing to do, which `systemctl` counts as done.
fn job_succeeded(result: &str) -> bool {
    matches!(result, "done" | "skipped")
}

/// Why a job did not finish as asked, in the words `systemctl` uses,
/// with the service's own result (`service_result`) explaining a
/// failure where it can.
fn job_failure(unit: &str, result: &str, service_result: Option<&str>) -> String {
    match result {
        "canceled" => format!("the request for {unit} was canceled"),
        "timeout" => format!("the request for {unit} timed out"),
        "dependency" => format!("a unit {unit} depends on failed to start"),
        "invalid" => format!("{unit} is not active"),
        "assert" => format!("an assertion failed for {unit}"),
        "unsupported" => format!("{unit} cannot do that on this system"),
        "collected" => format!("the request for {unit} was dropped before it ran"),
        "once" => format!("{unit} was started once already and cannot be started again"),
        "frozen" => format!("{unit} is frozen"),
        "concurrency" => format!("{unit}'s slice runs as many units as it may"),
        _ => {
            let because = service_result
                .and_then(explain_service_result)
                .map_or_else(String::new, |reason| format!(" because {reason}"));
            format!(
                "{unit} failed{because} (\"systemctl --user status {unit}\" and \
                 \"journalctl --user -xeu {unit}\" have the details)"
            )
        }
    }
}

/// `systemctl`'s explanation of a service's `Result`.
fn explain_service_result(result: &str) -> Option<&'static str> {
    Some(match result {
        "resources" => "of unavailable resources or another system error",
        "protocol" => "the service did not take the steps its unit configuration requires",
        "timeout" => "a timeout was exceeded",
        "exit-code" => "the control process exited with an error code",
        "signal" => "a fatal signal was delivered to the control process",
        "core-dump" => "a fatal signal made the control process dump core",
        "watchdog" => "the service failed to send a watchdog ping",
        "start-limit-hit" => "it was started too often in a short time",
        "oom-kill" => "it ran out of memory",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::Message;
    use zbus::names::OwnedErrorName;
    use zbus::zvariant::Value;

    #[test]
    fn units_are_found_at_the_path_systemd_escapes_their_name_to() {
        assert_eq!(
            object_path("xremap.service"),
            "/org/freedesktop/systemd1/unit/xremap_2eservice"
        );
        assert_eq!(
            object_path("systemd-journald.service"),
            "/org/freedesktop/systemd1/unit/systemd_2djournald_2eservice"
        );
        assert_eq!(
            object_path("user@1000.service"),
            "/org/freedesktop/systemd1/unit/user_401000_2eservice",
            "digits stay unless they come first"
        );
        assert_eq!(
            object_path("1a.service"),
            "/org/freedesktop/systemd1/unit/_31a_2eservice"
        );
        assert_eq!(object_path(""), "/org/freedesktop/systemd1/unit/_");
    }

    fn properties(pairs: &[(&str, Value<'_>)]) -> HashMap<String, OwnedValue> {
        pairs
            .iter()
            .map(|(name, value)| {
                (
                    (*name).to_owned(),
                    OwnedValue::try_from(value.try_clone().unwrap()).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn the_unit_properties_are_read_from_get_all() {
        let state = UnitState::from_properties(&properties(&[
            ("Id", Value::from("xremap.service")),
            ("LoadState", Value::from("loaded")),
            ("ActiveState", Value::from("activating")),
            ("SubState", Value::from("auto-restart")),
            ("UnitFileState", Value::from("enabled")),
            (
                "FragmentPath",
                Value::from("/home/me/.config/systemd/user/xremap.service"),
            ),
            ("NRestarts", Value::from(3_u32)),
        ]));
        assert_eq!(
            state,
            UnitState {
                load_state: "loaded".to_owned(),
                active_state: "activating".to_owned(),
                sub_state: "auto-restart".to_owned(),
                unit_file_state: "enabled".to_owned(),
                fragment_path: "/home/me/.config/systemd/user/xremap.service".to_owned(),
            }
        );

        // Missing, or not text: empty, which reads as not known.
        let state = UnitState::from_properties(&properties(&[
            ("LoadState", Value::from("not-found")),
            ("ActiveState", Value::from(7_u32)),
        ]));
        assert_eq!(state.load_state, "not-found");
        assert_eq!(state.active_state, "");
        assert_eq!(state.fragment_path, "");
    }

    fn method_error(name: &str, description: &str) -> zbus::Error {
        let reply = Message::method_call("/", "Ping")
            .unwrap()
            .build(&())
            .unwrap();
        zbus::Error::MethodError(
            OwnedErrorName::try_from(name).unwrap(),
            Some(description.to_owned()),
            reply,
        )
    }

    #[test]
    fn systemds_refusals_keep_its_explanation() {
        let err = Error::from(method_error(
            "org.freedesktop.systemd1.NoSuchUnit",
            "Unit xremap.service not found.",
        ));
        assert!(matches!(&err, Error::Refused(_)), "{err:?}");
        assert_eq!(err.to_string(), "unit xremap.service not found");
    }

    #[test]
    fn nobody_answering_for_systemd_means_it_is_unavailable() {
        for name in NO_MANAGER {
            let err = Error::from(method_error(name, "The name is not activatable"));
            assert!(matches!(err, Error::Unavailable(_)), "{name}");
            assert!(
                err.to_string()
                    .starts_with("could not talk to systemd's user manager: "),
                "{err}"
            );
        }
        let err = Error::from(zbus::Error::InputOutput(Arc::new(io::Error::from(
            io::ErrorKind::NotFound,
        ))));
        assert!(matches!(err, Error::Unavailable(_)));
    }

    #[test]
    fn messages_are_made_to_follow_a_prefix() {
        assert_eq!(
            clause("Unit xremap.service is masked."),
            "unit xremap.service is masked"
        );
        assert_eq!(clause("Access denied"), "access denied");
        assert_eq!(
            clause("D-Bus connection closed."),
            "D-Bus connection closed",
            "an acronym keeps its capitals"
        );
        assert_eq!(clause("already lowercase"), "already lowercase");
        assert_eq!(clause(""), "");
    }

    #[test]
    fn only_a_finished_or_needless_job_succeeded() {
        assert!(job_succeeded("done"));
        assert!(job_succeeded("skipped"));
        for result in ["failed", "canceled", "timeout", "dependency", "invalid"] {
            assert!(!job_succeeded(result), "{result}");
        }
    }

    #[test]
    fn failed_jobs_are_explained_as_systemctl_would() {
        assert_eq!(
            job_failure("xremap.service", "failed", Some("exit-code")),
            "xremap.service failed because the control process exited with an error code \
             (\"systemctl --user status xremap.service\" and \
             \"journalctl --user -xeu xremap.service\" have the details)"
        );
        assert_eq!(
            job_failure("xremap.service", "failed", Some("start-limit-hit")),
            "xremap.service failed because it was started too often in a short time \
             (\"systemctl --user status xremap.service\" and \
             \"journalctl --user -xeu xremap.service\" have the details)"
        );
        assert!(
            job_failure("xremap.service", "failed", None)
                .starts_with("xremap.service failed (\"systemctl"),
            "without a known result there is only the pointer to the logs"
        );
        assert_eq!(
            job_failure("xremap.service", "canceled", Some("exit-code")),
            "the request for xremap.service was canceled"
        );
        assert_eq!(
            job_failure("xremap.service", "dependency", None),
            "a unit xremap.service depends on failed to start"
        );
    }

    /// A scratch unit in the user's own systemd, removed again when the
    /// test ends. Only transient units named `keyloom-test-…` are
    /// touched; the remapping service never is.
    struct ScratchUnit(String);

    impl ScratchUnit {
        /// Start `command` as a transient unit, waiting until it runs.
        fn start(label: &str, properties: &[String], command: &[&str]) -> Self {
            let name = format!("keyloom-test-{label}-{}.service", std::process::id());
            let status = std::process::Command::new("systemd-run")
                .args(["--user", "--quiet", "--unit", &name])
                .args(
                    properties
                        .iter()
                        .map(|property| format!("--property={property}")),
                )
                .args(command)
                .status()
                .expect("systemd-run");
            assert!(status.success(), "systemd-run could not start {name}");
            Self(name)
        }
    }

    impl Drop for ScratchUnit {
        fn drop(&mut self) {
            // A transient unit is gone once stopped, unless it failed.
            for action in ["stop", "reset-failed"] {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", action, &self.0])
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
    }

    /// The next change announced within a second, if any.
    async fn next_change(changes: &mut Changes) -> Option<Change> {
        tokio::time::timeout(std::time::Duration::from_secs(1), changes.next())
            .await
            .ok()
            .flatten()
    }

    #[tokio::test]
    #[ignore = "starts and stops scratch units in the user's systemd; run by hand"]
    async fn jobs_and_changes_against_the_running_user_manager() {
        let manager = Manager::session().await.expect("a session bus");
        let unit = ScratchUnit::start("jobs", &[], &["/bin/sleep", "60"]);
        let name = unit.0.as_str();
        let mut changes = manager.changes(name).await.expect("signals");
        manager.subscribe().await.expect("subscribed");
        manager
            .subscribe()
            .await
            .expect("subscribing twice is fine");

        let (state, first) = manager.unit(name).await.expect("state");
        assert_eq!(
            (state.load_state.as_str(), state.active_state.as_str()),
            ("loaded", "active")
        );
        assert!(state.fragment_path.ends_with(name), "{state:?}");

        // A restart succeeds once the job is done, which is after the
        // unit's own change of state was announced.
        manager.restart(name).await.expect("restarted");
        assert_eq!(next_change(&mut changes).await, Some(Change::Unit));
        let (state, second) = manager.unit(name).await.expect("state");
        assert_eq!(state.active_state, "active");
        assert!(second > first, "later replies are received later");

        manager
            .start(name)
            .await
            .expect("starting a running unit is fine");
        manager.stop(name).await.expect("stopped");
        let (state, _) = manager.unit(name).await.expect("state");
        assert_ne!(state.active_state, "active");

        // The collected unit is gone: systemd refuses, in its words.
        let err = manager.start(name).await.expect_err("no such unit");
        assert_eq!(err.to_string(), format!("unit {name} not found"));
        manager.reload().await.expect("reloaded");
        drop(unit);

        // A job that fails reports why: this unit's start check passes
        // once, so its restart fails.
        let dir = crate::testing::TempDir::new("systemd-failing-job");
        let marker = dir.path().join("started");
        let failing = ScratchUnit::start(
            "failing",
            &[format!(
                "ExecStartPre=/bin/mkdir {}",
                marker.to_str().expect("a UTF-8 temp path")
            )],
            &["/bin/sleep", "60"],
        );
        let err = manager.restart(&failing.0).await.expect_err("failed");
        assert!(
            err.to_string()
                .contains("failed because the control process exited with an error code"),
            "{err}"
        );
    }
}

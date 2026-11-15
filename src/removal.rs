//! Undoing first-run setup: taking out again what [`crate::setup`]
//! put on the system.
//!
//! Keyloom keeps no record of what it installed, so everything here is
//! recognised by what it is: the user unit by the marker on its first
//! line, the xremap download by where it is and its digest, and the
//! udev rule and module configuration by being exactly what setup
//! writes, at the paths it writes them to. Anything else (a
//! distribution's xremap, a unit the user wrote, an administrator's own
//! rules) is left alone. Each file is checked again at the moment it is
//! deleted, as the user or as the administrator, so one that changed
//! since the dialog opened is kept.
//!
//! Membership of the `input` group is different: setup may have added
//! it, but so may the user, for another program. It is only removed
//! when asked for.
//!
//! Profiles, and the configuration Keyloom generates from them, are not
//! part of setup and stay.

use std::io;
use std::path::{Path, PathBuf};

use crate::install;
use crate::service;
use crate::setup::{self, ActionError, INPUT_GROUP, RULE, RULES_PATH};

/// The module configuration setup writes so `uinput` loads at boot.
pub const MODULES_PATH: &str = "/etc/modules-load.d/uinput.conf";

/// What [`MODULES_PATH`] holds when setup wrote it.
const MODULE: &str = "uinput";

/// What there is to remove, from one look at the system.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Keyloom's own user unit.
    pub unit: Option<PathBuf>,
    /// The xremap Keyloom downloaded.
    pub xremap: Option<PathBuf>,
    /// The udev rule at [`RULES_PATH`] is the one setup writes.
    pub rule: bool,
    /// The module configuration at [`MODULES_PATH`] is the one setup
    /// writes.
    pub module: bool,
    /// The login name, when the user is listed in the `input` group.
    pub group_member: Option<String>,
}

impl Plan {
    /// Whether anything Keyloom installed is still there.
    pub fn has_installed(&self) -> bool {
        self.unit.is_some() || self.xremap.is_some() || self.rule || self.module
    }

    /// Whether removing would change anything, given the choice about
    /// the `input` group.
    pub fn has_work(&self, leave_group: bool) -> bool {
        self.has_installed() || self.leaves_group(leave_group)
    }

    /// Whether removing needs the administrator.
    pub fn needs_admin(&self, leave_group: bool) -> bool {
        self.rule || self.module || self.leaves_group(leave_group)
    }

    fn leaves_group(&self, leave_group: bool) -> bool {
        leave_group && self.group_member.is_some()
    }
}

/// Find out what Keyloom installed that is still there.
pub async fn probe() -> Plan {
    let unit_path = service::unit_path();
    let managed = install::managed_path();
    let (unit, xremap, rule, module, groups, status, passwd) = tokio::join!(
        async {
            match &unit_path {
                Some(path) => is_keyloom_unit(&tokio::fs::read(path).await.unwrap_or_default()),
                None => false,
            }
        },
        async {
            match &managed {
                Some(path) => setup::is_managed(path).await,
                None => false,
            }
        },
        setup::read_or_empty(RULES_PATH),
        setup::read_or_empty(MODULES_PATH),
        setup::read_or_empty("/etc/group"),
        setup::read_or_empty("/proc/self/status"),
        setup::read_or_empty("/etc/passwd"),
    );
    let group_member = setup::current_user(&status, &passwd).filter(|user| {
        setup::group_entry(&groups, INPUT_GROUP)
            .is_some_and(|(_, members)| members.contains(&user.as_str()))
    });
    Plan {
        unit: unit_path.filter(|_| unit),
        xremap: managed.filter(|_| xremap),
        rule: is_keyloom_rule(&rule),
        module: is_keyloom_module(&module),
        group_member,
    }
}

fn is_keyloom_unit(bytes: &[u8]) -> bool {
    bytes.starts_with(service::UNIT_MARKER.as_bytes())
}

/// Whether a rules file is exactly what setup writes: an
/// administrator's file with anything else in it is theirs.
fn is_keyloom_rule(text: &str) -> bool {
    text.trim_end_matches('\n') == RULE
}

fn is_keyloom_module(text: &str) -> bool {
    text.trim_end_matches('\n') == MODULE
}

/// A shell line that deletes `path` only while it holds exactly
/// `content`, give or take trailing newlines. `content` must not
/// contain a single quote.
fn remove_if_unchanged(path: &str, content: &str) -> String {
    format!(
        "if [ -f '{path}' ] && [ \"$(cat '{path}')\" = '{content}' ]; then rm -f '{path}'; fi\n"
    )
}

/// What runs as the administrator: delete the rule and the module
/// configuration while they are still what setup wrote, put the device
/// node back to its default access, and, when asked, take the user
/// (the script's first argument) out of the `input` group. The module
/// stays loaded until the next boot; other programs may be using it.
pub fn admin_script(plan: &Plan, leave_group: bool) -> String {
    let mut script = String::from("set -e\n");
    if plan.rule || plan.module {
        script.push_str(&remove_if_unchanged(RULES_PATH, RULE));
        script.push_str(&remove_if_unchanged(MODULES_PATH, MODULE));
        script.push_str(
            "udevadm control --reload-rules\n\
             udevadm trigger --subsystem-match=misc --sysname-match=uinput\n\
             udevadm settle\n",
        );
    }
    if plan.leaves_group(leave_group) {
        script.push_str("gpasswd -d \"$1\" ");
        script.push_str(INPUT_GROUP);
        script.push('\n');
    }
    script
}

/// Delete a file when `is_ours` says so of its contents. Reports
/// whether it was deleted; a file that is not there, or not ours, is
/// not an error.
fn remove_file_if(path: &Path, is_ours: impl FnOnce(&[u8]) -> bool) -> io::Result<bool> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };
    if !is_ours(&bytes) {
        return Ok(false);
    }
    std::fs::remove_file(path)?;
    Ok(true)
}

/// [`remove_file_if`] off the async threads, with the failure worded
/// for the dialog.
async fn remove_file(
    path: PathBuf,
    what: &'static str,
    is_ours: impl FnOnce(&[u8]) -> bool + Send + 'static,
) -> Result<(), ActionError> {
    let shown = path.display().to_string();
    let failed = move |err: &dyn std::fmt::Display| {
        ActionError::Failed(format!("could not remove {what} ({shown}): {err}"))
    };
    tokio::task::spawn_blocking(move || remove_file_if(&path, is_ours))
        .await
        .map_err(|err| failed(&err))?
        .map_err(|err| failed(&err))?;
    Ok(())
}

/// Stop the service, stop it starting at login, delete its file, and
/// have systemd forget it. systemd is only asked about a unit it knows.
async fn remove_service(path: PathBuf) -> Result<(), ActionError> {
    let status = service::inspect().await.status;
    if !matches!(
        status,
        service::Status::Unavailable | service::Status::NotFound
    ) {
        service::set(service::Remapping::Off).await?;
        service::disable().await?;
    }
    remove_file(path, "the service file", is_keyloom_unit).await?;
    if status != service::Status::Unavailable {
        service::daemon_reload().await?;
    }
    Ok(())
}

/// Carry out the plan. The administrator's part comes first: when the
/// prompt is dismissed or refused, nothing at all has changed. When it
/// cannot run or fails, the parts that need no administrator still go
/// ahead, and the failure is reported after them. Look again
/// ([`probe`]) afterwards for what is left.
///
/// # Errors
///
/// When authorization is cancelled or refused, a command fails,
/// systemd refuses, or a file cannot be deleted.
pub async fn run(plan: &Plan, leave_group: bool) -> Result<(), ActionError> {
    let admin = if plan.needs_admin(leave_group) {
        let script = admin_script(plan, leave_group);
        let mut args = vec!["-c", script.as_str(), "sh"];
        if leave_group && let Some(user) = &plan.group_member {
            args.push(user);
        }
        setup::privileged(Path::new("/bin/sh"), &args).await
    } else {
        Ok(())
    };
    if matches!(
        admin,
        Err(ActionError::Cancelled | ActionError::NotAuthorized)
    ) {
        return admin;
    }
    // The service goes before the binary it runs.
    let service = match plan.unit.clone() {
        Some(path) => remove_service(path).await,
        None => Ok(()),
    };
    let xremap = match plan.xremap.clone() {
        Some(path) => {
            remove_file(path, "xremap", |bytes| {
                install::known_release(bytes).is_some()
            })
            .await
        }
        None => Ok(()),
    };
    admin.and(service).and(xremap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    fn everything() -> Plan {
        Plan {
            unit: Some(PathBuf::from(
                "/home/blake/.config/systemd/user/xremap.service",
            )),
            xremap: Some(PathBuf::from("/home/blake/.local/bin/xremap")),
            rule: true,
            module: true,
            group_member: Some("blake".to_owned()),
        }
    }

    #[test]
    fn only_exactly_what_setup_writes_counts_as_keylooms() {
        // What `UINPUT_SCRIPT` prints into the two files.
        assert!(is_keyloom_rule(&format!("{RULE}\n")));
        assert!(is_keyloom_module("uinput\n"));
        assert!(
            setup::UINPUT_SCRIPT.contains(RULE),
            "setup writes the rule removal looks for"
        );
        assert!(setup::UINPUT_SCRIPT.contains(MODULES_PATH));

        assert!(!is_keyloom_rule(""));
        assert!(!is_keyloom_rule(&format!("# mine\n{RULE}\n")));
        assert!(!is_keyloom_rule(
            "KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\"\n"
        ));
        assert!(!is_keyloom_module(""));
        assert!(!is_keyloom_module("uinput\nloop\n"));

        assert!(is_keyloom_unit(
            format!("{}\n[Unit]\n", service::UNIT_MARKER).as_bytes()
        ));
        assert!(!is_keyloom_unit(b"[Unit]\nDescription=mine\n"));
        assert!(!is_keyloom_unit(b""));
    }

    #[test]
    fn the_input_group_is_left_unless_asked_for() {
        let plan = everything();
        assert!(plan.has_work(false));
        assert!(plan.needs_admin(false));
        assert!(!admin_script(&plan, false).contains("gpasswd"));
        assert!(admin_script(&plan, true).contains("gpasswd -d \"$1\" input\n"));

        // Only the membership is left: nothing happens without the
        // choice, and nobody to remove means nothing to do either way.
        let member_only = Plan {
            group_member: Some("blake".to_owned()),
            ..Plan::default()
        };
        assert!(!member_only.has_installed());
        assert!(!member_only.has_work(false));
        assert!(member_only.has_work(true));
        assert!(member_only.needs_admin(true));
        assert!(!admin_script(&member_only, true).contains("udevadm"));
        assert!(!Plan::default().has_work(true));
        assert!(!Plan::default().needs_admin(true));
    }

    #[test]
    fn only_the_users_own_files_need_no_administrator() {
        let plan = Plan {
            rule: false,
            module: false,
            ..everything()
        };
        assert!(plan.has_work(false));
        assert!(!plan.needs_admin(false));
        assert!(plan.needs_admin(true));
    }

    #[test]
    fn the_administrators_script_deletes_only_unchanged_files() {
        let dir = TempDir::new("removal-script");
        let ours = dir.path().join("ours.rules");
        let theirs = dir.path().join("theirs.rules");
        let missing = dir.path().join("missing.rules");
        std::fs::write(&ours, format!("{RULE}\n")).unwrap();
        std::fs::write(&theirs, format!("{RULE}\n# and more\n")).unwrap();

        let script: String = [&ours, &theirs, &missing]
            .iter()
            .map(|path| remove_if_unchanged(path.to_str().unwrap(), RULE))
            .collect();
        let status = std::process::Command::new("/bin/sh")
            .args(["-c", &format!("set -e\n{script}")])
            .status()
            .unwrap();

        assert!(status.success(), "a missing file is not a failure");
        assert!(!ours.exists());
        assert!(theirs.exists());
    }

    #[test]
    fn a_file_is_deleted_only_when_it_is_recognised() {
        let dir = TempDir::new("removal-files");
        let path = dir.path().join("xremap.service");

        assert!(!remove_file_if(&path, |_| true).unwrap(), "nothing there");

        std::fs::write(&path, "[Unit]\nDescription=mine\n").unwrap();
        assert!(!remove_file_if(&path, is_keyloom_unit).unwrap());
        assert!(path.exists(), "a unit the user wrote stays");

        std::fs::write(&path, format!("{}\n[Unit]\n", service::UNIT_MARKER)).unwrap();
        assert!(remove_file_if(&path, is_keyloom_unit).unwrap());
        assert!(!path.exists());
    }
}

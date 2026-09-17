//! First-run system setup: the checks and fixes that take a fresh
//! system to working remapping.
//!
//! Beyond Keyloom itself, remapping needs four things: the xremap
//! binary (found on `PATH`, or downloaded by Keyloom into the user's
//! `~/.local/bin` when there is none; see [`crate::install`]), read
//! access to keyboards (membership in the `input` group), write access
//! to `/dev/uinput` for the virtual keyboard xremap types on (a udev
//! rule, plus the `uinput` module), and a systemd user unit that runs
//! xremap with Keyloom's generated configuration, told which desktop to
//! ask for application-specific rules. [`probe`] finds out where the
//! system stands on each; the action functions fix one step at a time,
//! asking for administrator authorization through the desktop's polkit
//! prompt (`pkexec`) where a change needs it. Keyloom itself stays
//! unprivileged throughout.

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::install;
use crate::service;
use crate::session::{Desktop, Session};
use crate::xremap;

/// The group Linux grants keyboard (`/dev/input`) access to.
pub const INPUT_GROUP: &str = "input";

/// The udev rules file Keyloom installs, named like the one the xremap
/// packages ship so the two never disagree.
pub const RULES_FILE: &str = "00-xremap-input.rules";

/// Where [`RULES_FILE`] is written: the administrator's rules directory.
pub const RULES_PATH: &str = "/etc/udev/rules.d/00-xremap-input.rules";

/// Directories udev reads rules from; a rules file in any of them counts.
const RULES_DIRS: &[&str] = &[
    "/etc/udev/rules.d",
    "/run/udev/rules.d",
    "/usr/local/lib/udev/rules.d",
    "/usr/lib/udev/rules.d",
    "/lib/udev/rules.d",
];

/// The rule itself: the `input` group and the seat's active session may
/// open `/dev/uinput`.
pub const RULE: &str = r#"KERNEL=="uinput", GROUP="input", TAG+="uaccess""#;

/// The device xremap creates its virtual keyboard through.
pub const UINPUT: &str = "/dev/uinput";

/// Where to get xremap when it is not installed.
pub const XREMAP_URL: &str = "https://github.com/xremap/xremap#installation";

/// xremap's GNOME Shell extension, which its GNOME client asks for the
/// window in front; without it, application-specific rules cannot
/// match on GNOME's Wayland session.
pub const XREMAP_GNOME_EXTENSION_URL: &str = "https://extensions.gnome.org/extension/5060/xremap/";

/// xremap's own guide to the permissions setup grants, for systems
/// Keyloom cannot set up itself.
pub const XREMAP_NO_SUDO_URL: &str =
    "https://github.com/xremap/xremap/blob/master/doc/running_without_sudo.md";

/// What runs as the administrator to prepare `/dev/uinput`: install the
/// rule, make sure the module is loaded now and at boot, and apply the
/// rule to the existing device node.
pub const UINPUT_SCRIPT: &str = "set -e\n\
    printf '%s\\n' 'KERNEL==\"uinput\", GROUP=\"input\", TAG+=\"uaccess\"' > /etc/udev/rules.d/00-xremap-input.rules\n\
    mkdir -p /etc/modules-load.d\n\
    printf 'uinput\\n' > /etc/modules-load.d/uinput.conf\n\
    modprobe uinput\n\
    udevadm control --reload-rules\n\
    udevadm trigger --subsystem-match=misc --sysname-match=uinput\n\
    udevadm settle\n";

/// [`UINPUT_SCRIPT`] as commands to paste into a terminal, for doing
/// the step by hand.
pub fn uinput_commands() -> String {
    format!(
        "echo '{RULE}' | sudo tee {RULES_PATH}\n\
         sudo mkdir -p /etc/modules-load.d\n\
         echo uinput | sudo tee /etc/modules-load.d/uinput.conf\n\
         sudo modprobe uinput\n\
         sudo udevadm control --reload-rules\n\
         sudo udevadm trigger --subsystem-match=misc --sysname-match=uinput"
    )
}

/// The setup steps, in the order the wizard walks them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The xremap binary is installed.
    Xremap,
    /// The user may read keyboards.
    InputGroup,
    /// The user may create a virtual keyboard.
    Uinput,
    /// A user service runs xremap with Keyloom's configuration.
    Service,
}

impl Step {
    /// Every step, in wizard order.
    pub const ALL: [Self; 4] = [Self::Xremap, Self::InputGroup, Self::Uinput, Self::Service];

    /// Position in [`Self::ALL`].
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|step| *step == self)
            .unwrap_or_default()
    }

    /// The step after this one, if any.
    pub fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() + 1).copied()
    }

    /// The step before this one, if any.
    pub fn previous(self) -> Option<Self> {
        self.index().checked_sub(1).map(|index| Self::ALL[index])
    }
}

/// Whether the xremap binary could be found, and what it can do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XremapCheck {
    Found {
        path: PathBuf,
        /// What `xremap --version` reported, when it ran.
        version: Option<String>,
        /// The desktops this build can ask which window is in front,
        /// from `xremap --list-desktops`. `None` when the binary has no
        /// such flag (before xremap 0.15.13) or its answer was not
        /// understood.
        desktops: Option<Vec<Desktop>>,
        /// Keyloom's own download: at [`install::managed_path`] and,
        /// byte for byte, a release Keyloom ships. A binary the user put
        /// there themselves is not.
        managed: bool,
    },
    /// Neither on `PATH` nor where Keyloom would have put it.
    Missing,
}

impl XremapCheck {
    /// The binary, when it was found.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Found { path, .. } => Some(path),
            Self::Missing => None,
        }
    }
}

/// The user's standing with the `input` group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupCheck {
    /// Membership is in effect for this session.
    Effective,
    /// The user is listed in the group, but this session started
    /// before that: a new login picks it up.
    NeedsLogin,
    /// Not a member.
    NotMember,
    /// This system has no `input` group at all.
    NoGroup,
}

/// Whether the virtual keyboard device can be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UinputCheck {
    /// `/dev/uinput` opens for writing.
    Writable,
    /// The node exists but this session may not write to it.
    NotWritable { rule_installed: bool },
    /// No `/dev/uinput`: the `uinput` module is not loaded.
    Missing { rule_installed: bool },
}

/// What kind of `xremap.service` user unit exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitCheck {
    /// Keyloom's own unit, exactly as it would be generated now.
    Keyloom { active: bool, enabled: bool },
    /// Keyloom's own unit, but not what it would generate now (the
    /// binary or the configuration moved).
    Stale { active: bool },
    /// A unit Keyloom did not write.
    Foreign {
        /// Its `ExecStart` line(s), for the user to recognize.
        exec_start: String,
        /// Whether it names Keyloom's generated configuration.
        reads_config: bool,
        active: bool,
        /// Where systemd loaded it from.
        path: Option<PathBuf>,
    },
    /// No such unit.
    Missing,
    /// No systemd user session to ask.
    Unavailable,
}

/// What the service step can do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceAction {
    /// Write Keyloom's unit, enable it, and start it when access allows.
    Install,
    /// Rewrite Keyloom's own unit to the current binary and config.
    Update,
    /// Replace a foreign unit (backing its file up) with Keyloom's.
    Replace,
    /// Enable Keyloom's unit at login.
    Enable,
    /// Start a unit that already reads Keyloom's configuration.
    Start,
}

/// What the xremap step can do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XremapAction {
    /// Download the release Keyloom ships into the user's `~/.local/bin`.
    Download,
    /// Replace Keyloom's own download with the release it ships now.
    Update,
}

/// Whether application-specific remaps can work here: whether the
/// installed xremap can tell which window is in front on this desktop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppMatching {
    /// No xremap to ask.
    NotInstalled,
    /// The build has a client for this desktop.
    Supported(Desktop),
    /// The build has no client for this desktop; `supports` lists the
    /// desktops it does have one for.
    Unsupported {
        desktop: Desktop,
        supports: Vec<Desktop>,
    },
    /// The build does not say which desktops it supports (before xremap
    /// 0.15.13), so it picks on its own.
    Unreported,
    /// Keyloom could not tell which desktop this is, so xremap picks on
    /// its own.
    UnknownDesktop,
}

/// Everything setup learned about the system in one pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facts {
    /// The login name, for the group change.
    pub user: Option<String>,
    /// The xremap binary.
    pub xremap: XremapCheck,
    /// Read access to keyboards.
    pub group: GroupCheck,
    /// Write access to the virtual keyboard device.
    pub uinput: UinputCheck,
    /// The user service.
    pub unit: UnitCheck,
    /// Keyloom's generated configuration, which the unit must read.
    pub config: Option<PathBuf>,
    /// The desktop this session runs on, which the unit tells xremap.
    pub session: Session,
}

impl Facts {
    /// Whether xremap could open keyboards and create its output device
    /// from this session right now.
    pub fn has_effective_access(&self) -> bool {
        self.group == GroupCheck::Effective && self.uinput == UinputCheck::Writable
    }

    /// The installed xremap binary, if there is one.
    pub fn xremap_path(&self) -> Option<&Path> {
        self.xremap.path()
    }

    /// How the unit should start this xremap on this session.
    pub fn launch(&self) -> service::Launch {
        launch_for(&self.xremap, self.session)
    }

    /// The change the xremap step offers, if any: a download when there
    /// is no xremap at all, an update when Keyloom's own download is not
    /// the release it ships now. Both need a release for this processor
    /// and a home directory to put it in. A copy the user installed
    /// themselves is never touched, wherever it is.
    pub fn xremap_action(&self) -> Option<XremapAction> {
        if install::asset().is_none() || install::managed_path().is_none() {
            return None;
        }
        match &self.xremap {
            XremapCheck::Missing => Some(XremapAction::Download),
            XremapCheck::Found {
                managed: true,
                version,
                ..
            } if version.as_deref() != Some(install::RELEASE) => Some(XremapAction::Update),
            XremapCheck::Found { .. } => None,
        }
    }

    /// Whether application-specific remaps can work with this xremap on
    /// this desktop.
    pub fn app_matching(&self) -> AppMatching {
        let XremapCheck::Found { desktops, .. } = &self.xremap else {
            return AppMatching::NotInstalled;
        };
        let Some(desktop) = self.session.desktop else {
            return AppMatching::UnknownDesktop;
        };
        match desktops {
            None => AppMatching::Unreported,
            Some(supports) if supports.contains(&desktop) => AppMatching::Supported(desktop),
            Some(supports) => AppMatching::Unsupported {
                desktop,
                supports: supports.clone(),
            },
        }
    }

    /// Whether the step is in order.
    pub fn is_step_ok(&self, step: Step) -> bool {
        match step {
            Step::Xremap => self.xremap_path().is_some(),
            Step::InputGroup => self.group == GroupCheck::Effective,
            Step::Uinput => self.uinput == UinputCheck::Writable,
            Step::Service => match &self.unit {
                UnitCheck::Keyloom { active, enabled } => *active && *enabled,
                UnitCheck::Foreign {
                    reads_config,
                    active,
                    ..
                } => *reads_config && *active,
                UnitCheck::Stale { .. } | UnitCheck::Missing | UnitCheck::Unavailable => false,
            },
        }
    }

    /// Whether the step is done except for a new login taking effect.
    pub fn step_needs_login(&self, step: Step) -> bool {
        match step {
            Step::Xremap => false,
            Step::InputGroup => self.group == GroupCheck::NeedsLogin,
            // The rule grants access through the input group, so a login
            // helps only while joining the group waits for one. With
            // membership in effect, it is the rule that isn't working.
            Step::Uinput => {
                matches!(
                    self.uinput,
                    UinputCheck::NotWritable {
                        rule_installed: true
                    }
                ) && self.group == GroupCheck::NeedsLogin
            }
            Step::Service => {
                !self.has_effective_access()
                    && matches!(
                        self.unit,
                        UnitCheck::Keyloom {
                            active: false,
                            enabled: true
                        }
                    )
            }
        }
    }

    /// Whether the step is either in order or only waiting for a login.
    pub fn is_step_settled(&self, step: Step) -> bool {
        self.is_step_ok(step) || self.step_needs_login(step)
    }

    /// Every step in order: remapping works now.
    pub fn is_all_ok(&self) -> bool {
        Step::ALL.iter().all(|step| self.is_step_ok(*step))
    }

    /// Every step settled: nothing left but a new login, if that.
    pub fn is_configured(&self) -> bool {
        Step::ALL.iter().all(|step| self.is_step_settled(*step))
    }

    /// Whether Keyloom can fix the step itself: the fix its page offers
    /// is the only one setup runs.
    pub fn can_fix(&self, step: Step) -> bool {
        match step {
            Step::Xremap => self.xremap_action().is_some(),
            Step::InputGroup => self.group == GroupCheck::NotMember && self.user.is_some(),
            Step::Uinput => !self.is_step_settled(step),
            Step::Service => self.service_action().is_some(),
        }
    }

    /// Whether Keyloom can save its service file for the user to turn
    /// on by hand: there is no service yet, and something to point one
    /// at.
    pub fn can_save_service(&self) -> bool {
        self.service_action() == Some(ServiceAction::Install)
    }

    /// Whether setup should stop at the step on its way forward: it is
    /// not settled, or it offers an update. Setup passes over the rest.
    pub fn wants_attention(&self, step: Step) -> bool {
        !self.is_step_settled(step) || (step == Step::Xremap && self.xremap_action().is_some())
    }

    /// The first step after `after` (from the first step when `None`)
    /// that wants attention; `None` when none of the rest do.
    pub fn next_attention(&self, after: Option<Step>) -> Option<Step> {
        let start = after.map_or(0, |step| step.index() + 1);
        Step::ALL
            .iter()
            .skip(start)
            .copied()
            .find(|step| self.wants_attention(*step))
    }

    /// The change the service step offers, if any. Installing needs the
    /// xremap binary and a config path to point the unit at.
    pub fn service_action(&self) -> Option<ServiceAction> {
        let installable = self.xremap_path().is_some() && self.config.is_some();
        match &self.unit {
            UnitCheck::Missing => installable.then_some(ServiceAction::Install),
            UnitCheck::Stale { .. } => installable.then_some(ServiceAction::Update),
            UnitCheck::Foreign {
                reads_config: false,
                ..
            } => installable.then_some(ServiceAction::Replace),
            UnitCheck::Foreign {
                reads_config: true,
                active: false,
                ..
            } => Some(ServiceAction::Start),
            UnitCheck::Keyloom {
                active: false,
                enabled: true,
            } => {
                // Waiting for a login is not something starting fixes.
                (self.has_effective_access() && installable).then_some(ServiceAction::Start)
            }
            UnitCheck::Keyloom { enabled: false, .. } => {
                installable.then_some(ServiceAction::Enable)
            }
            UnitCheck::Keyloom {
                active: true,
                enabled: true,
            }
            | UnitCheck::Foreign {
                reads_config: true,
                active: true,
                ..
            }
            | UnitCheck::Unavailable => None,
        }
    }
}

/// Find out where the system stands on every step.
pub async fn probe() -> Facts {
    // Everything that depends on nothing else is looked up at the same
    // time: this process's own status, the user and group databases,
    // the udev rules, the binary, and the service.
    let (status, groups, passwd, rule_installed, xremap, unit) = tokio::join!(
        read_or_empty("/proc/self/status"),
        read_or_empty("/etc/group"),
        read_or_empty("/etc/passwd"),
        rules_installed(),
        xremap_check(),
        service::inspect(),
    );
    let user = current_user(&status, &passwd);
    let group = group_check(&groups, &status, user.as_deref());
    let uinput = uinput_check_at(Path::new(UINPUT), rule_installed).await;
    let config = xremap::config_path();
    let session = Session::detect();
    let launch = launch_for(&xremap, session);
    let expected = xremap
        .path()
        .zip(config.as_deref())
        .map(|(binary, config)| service::unit_file(binary, config, launch));
    let unit = unit_check(unit, expected.as_deref(), config.as_deref()).await;
    Facts {
        user,
        xremap,
        group,
        uinput,
        unit,
        config,
        session,
    }
}

/// How to start a given xremap on a given session: name the desktop
/// only when the binary lists it (a binary without `--list-desktops`
/// would refuse `--desktop` altogether, and one without this desktop's
/// client would ask nothing at all), and wait for a Wayland socket only
/// where one will appear.
pub fn launch_for(xremap: &XremapCheck, session: Session) -> service::Launch {
    let supported = match xremap {
        XremapCheck::Found {
            desktops: Some(desktops),
            ..
        } => Some(desktops.as_slice()),
        XremapCheck::Found { desktops: None, .. } | XremapCheck::Missing => None,
    };
    launch_with(supported, session)
}

fn launch_with(supported: Option<&[Desktop]>, session: Session) -> service::Launch {
    let desktop = session
        .desktop
        .filter(|desktop| supported.is_some_and(|supported| supported.contains(desktop)));
    service::Launch {
        desktop,
        wait_for_wayland: !session.x11,
    }
}

/// The binary and the way to start it, for running xremap outside the
/// unit (the application picker's `--list-windows`): the same binary
/// the unit would run, told about the same desktop.
pub async fn xremap_invocation() -> Option<(PathBuf, service::Launch)> {
    let path = locate_xremap().await?;
    let desktops = list_desktops(&path).await;
    let launch = launch_with(desktops.as_deref(), Session::detect());
    Some((path, launch))
}

/// A text file's contents, or nothing when it cannot be read: every
/// check treats an unreadable file as nothing known.
async fn read_or_empty(path: &str) -> String {
    tokio::fs::read_to_string(path).await.unwrap_or_default()
}

/// Look for the xremap binary and ask it what it is: its version, and
/// which desktops it can ask which window is in front. Both questions
/// make xremap exit before it touches any input device.
async fn xremap_check() -> XremapCheck {
    let Some(path) = locate_xremap().await else {
        return XremapCheck::Missing;
    };
    let (version, desktops, managed) =
        tokio::join!(version_of(&path), list_desktops(&path), is_managed(&path));
    XremapCheck::Found {
        path,
        version,
        desktops,
        managed,
    }
}

/// The xremap binary: the first on `PATH`, else Keyloom's own download,
/// which need not be on `PATH` at all.
pub async fn locate_xremap() -> Option<PathBuf> {
    locate_in(
        std::env::var_os("PATH").as_deref(),
        install::managed_path().as_deref(),
    )
    .await
}

/// [`locate_xremap`] over an explicit `PATH` value and download location.
async fn locate_in(path: Option<&OsStr>, managed: Option<&Path>) -> Option<PathBuf> {
    if let Some(found) = find_on_path("xremap", path).await {
        return Some(found);
    }
    match managed {
        Some(managed) if is_executable(managed).await => Some(managed.to_path_buf()),
        _ => None,
    }
}

/// What `xremap --version` reports, when it runs.
async fn version_of(path: &Path) -> Option<String> {
    Command::new(path)
        .arg("--version")
        .output()
        .await
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| parse_version(&String::from_utf8_lossy(&output.stdout)))
}

/// The desktops a binary can ask, from `xremap --list-desktops`. A
/// binary without the flag exits with an error, and an answer in an
/// unexpected shape is not guessed at: both give `None`.
async fn list_desktops(path: &Path) -> Option<Vec<Desktop>> {
    let output = Command::new(path)
        .arg("--list-desktops")
        .output()
        .await
        .ok()
        .filter(|output| output.status.success())?;
    parse_desktops(&String::from_utf8_lossy(&output.stdout))
}

/// Read `This variant of xremap supports: GNOME, KDE, …`. Names that
/// are not desktops (the `Socket` bridge) are skipped; without the
/// line, nothing is known.
pub(crate) fn parse_desktops(stdout: &str) -> Option<Vec<Desktop>> {
    let list = stdout
        .lines()
        .find_map(|line| line.split_once("supports:").map(|(_, list)| list))?;
    Some(
        list.split(',')
            .filter_map(Desktop::from_list_name)
            .collect(),
    )
}

/// Whether a binary is Keyloom's own download: where Keyloom puts it,
/// and byte for byte a release Keyloom ships. Hashing a few megabytes
/// only happens for a binary at that one path.
async fn is_managed(path: &Path) -> bool {
    let Some(managed) = install::managed_path() else {
        return false;
    };
    if !same_file(path, &managed).await {
        return false;
    }
    let Ok(bytes) = tokio::fs::read(path).await else {
        return false;
    };
    tokio::task::spawn_blocking(move || install::known_release(&bytes).is_some())
        .await
        .unwrap_or(false)
}

/// Whether two paths name the same file, following symlinks where the
/// paths resolve.
async fn same_file(a: &Path, b: &Path) -> bool {
    match tokio::join!(tokio::fs::canonicalize(a), tokio::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// The first executable named `name` in a `PATH`-style list.
async fn find_on_path(name: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    for dir in std::env::split_paths(path?).filter(|dir| !dir.as_os_str().is_empty()) {
        let candidate = dir.join(name);
        if is_executable(&candidate).await {
            return Some(candidate);
        }
    }
    None
}

async fn is_executable(path: &Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// The version out of `xremap 0.15.12`.
fn parse_version(output: &str) -> Option<String> {
    let version = output.trim().strip_prefix("xremap")?.trim();
    (!version.is_empty()).then(|| version.to_owned())
}

/// The login name of the real user this process runs as, from a
/// `/proc/<pid>/status` document and a `passwd`-format database,
/// falling back to `$USER`.
fn current_user(status: &str, passwd: &str) -> Option<String> {
    real_uid(status)
        .and_then(|uid| user_name(passwd, uid))
        .or_else(|| std::env::var("USER").ok().filter(|user| !user.is_empty()))
}

/// The real uid from a `/proc/<pid>/status` document.
fn real_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// The login name for a uid in a `passwd`-format database.
fn user_name(passwd: &str, uid: u32) -> Option<String> {
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        fields.next()?;
        (fields.next()?.parse::<u32>().ok()? == uid).then(|| name.to_owned())
    })
}

/// One entry of a `group`-format database: its gid and member names.
fn group_entry<'a>(database: &'a str, name: &str) -> Option<(u32, Vec<&'a str>)> {
    database.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != name {
            return None;
        }
        fields.next()?;
        let gid = fields.next()?.parse().ok()?;
        let members = fields
            .next()
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|member| !member.is_empty())
            .collect();
        Some((gid, members))
    })
}

/// The group ids in effect for a process: its real gid plus the
/// supplementary groups, from a `/proc/<pid>/status` document.
fn effective_gids(status: &str) -> Vec<u32> {
    let mut gids = Vec::new();
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Gid:") {
            // Real, effective, saved, and filesystem gid; membership is
            // about the real one.
            gids.extend(
                rest.split_whitespace()
                    .take(1)
                    .filter_map(|id| id.parse::<u32>().ok()),
            );
        } else if let Some(rest) = line.strip_prefix("Groups:") {
            gids.extend(
                rest.split_whitespace()
                    .filter_map(|id| id.parse::<u32>().ok()),
            );
        }
    }
    gids
}

/// Decide the group step from the group database, this process's
/// status, and the login name.
fn group_check(database: &str, status: &str, user: Option<&str>) -> GroupCheck {
    let Some((gid, members)) = group_entry(database, INPUT_GROUP) else {
        return GroupCheck::NoGroup;
    };
    if effective_gids(status).contains(&gid) {
        return GroupCheck::Effective;
    }
    if user.is_some_and(|user| members.contains(&user)) {
        GroupCheck::NeedsLogin
    } else {
        GroupCheck::NotMember
    }
}

/// Whether any udev rules directory has a rule about the uinput node.
async fn rules_installed() -> bool {
    for dir in RULES_DIRS {
        if rule_present(&Path::new(dir).join(RULES_FILE)).await {
            return true;
        }
    }
    false
}

/// Decide the uinput step by trying to open the device for writing,
/// which creates nothing until a device is actually registered.
async fn uinput_check_at(device: &Path, rule_installed: bool) -> UinputCheck {
    match tokio::fs::OpenOptions::new().write(true).open(device).await {
        Ok(_) => UinputCheck::Writable,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            UinputCheck::Missing { rule_installed }
        }
        Err(_) => UinputCheck::NotWritable { rule_installed },
    }
}

/// Whether a rules file exists and has a rule about the uinput node.
async fn rule_present(path: &Path) -> bool {
    tokio::fs::read_to_string(path)
        .await
        .is_ok_and(|text| rule_text_covers_uinput(&text))
}

fn rule_text_covers_uinput(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .any(|line| line.contains("uinput"))
}

/// Classify the unit systemd reported. `expected` is the unit file
/// Keyloom would write now (absent when it could not be generated).
async fn unit_check(
    unit: service::Unit,
    expected: Option<&str>,
    config: Option<&Path>,
) -> UnitCheck {
    let active = match unit.status {
        service::Status::Unavailable => return UnitCheck::Unavailable,
        service::Status::NotFound => return UnitCheck::Missing,
        service::Status::Active => true,
        service::Status::Inactive | service::Status::Failed => false,
    };
    let text = match unit.fragment_path.as_deref() {
        Some(path) => tokio::fs::read_to_string(path).await.ok(),
        None => None,
    };
    classify_unit(
        active,
        unit.enabled,
        unit.fragment_path,
        text.as_deref(),
        expected,
        config,
    )
}

fn classify_unit(
    active: bool,
    enabled: bool,
    path: Option<PathBuf>,
    text: Option<&str>,
    expected: Option<&str>,
    config: Option<&Path>,
) -> UnitCheck {
    match text {
        Some(text) if text.starts_with(service::UNIT_MARKER) => {
            if expected == Some(text) {
                UnitCheck::Keyloom { active, enabled }
            } else {
                UnitCheck::Stale { active }
            }
        }
        Some(text) => {
            // Only the startup command counts: a path named in a
            // comment, a commented-out ExecStart, or a setting that
            // never runs does not load anything.
            let exec_start = exec_start_of(text);
            UnitCheck::Foreign {
                reads_config: reads_config(&exec_start, config),
                exec_start,
                active,
                path,
            }
        }
        // A unit whose file cannot be read is not one Keyloom wrote.
        None => UnitCheck::Foreign {
            exec_start: String::new(),
            reads_config: false,
            active,
            path,
        },
    }
}

/// The `ExecStart=` value(s) of a unit file, continuation lines joined,
/// one per line; empty when there is none.
pub fn exec_start_of(text: &str) -> String {
    let mut lines = text.lines();
    let mut found: Vec<String> = Vec::new();
    while let Some(line) = lines.next() {
        let Some(value) = line.trim_start().strip_prefix("ExecStart=") else {
            continue;
        };
        let mut value = value.trim().to_owned();
        while let Some(stripped) = value.strip_suffix('\\') {
            value = stripped.trim_end().to_owned();
            let Some(next) = lines.next() else {
                break;
            };
            value.push(' ');
            value.push_str(next.trim());
        }
        if value.is_empty() {
            // An empty assignment resets the list.
            found.clear();
        } else {
            found.push(value);
        }
    }
    found.join("\n")
}

/// Whether a unit's startup command names Keyloom's generated
/// configuration. Takes the `ExecStart` value from [`exec_start_of`],
/// never the whole unit file: a path mentioned anywhere else in the
/// file is not one xremap reads.
///
/// The second test carries the rest: Keyloom always generates into
/// `xremap/keyloom.yml`, so a command spelling the directory some
/// other way (`%h`, `%E`, `$HOME`) still names the same file.
fn reads_config(exec_start: &str, config: Option<&Path>) -> bool {
    config.is_some_and(|config| exec_start.contains(&*config.to_string_lossy()))
        || exec_start.contains("xremap/keyloom.yml")
}

/// Why a fix did not happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionError {
    /// The user dismissed the authentication prompt.
    Cancelled,
    /// The desktop refused to authorize the change.
    NotAuthorized,
    /// `pkexec` is not installed, so nothing can ask for authorization.
    NoPolkit,
    /// The change ran and failed; the text is its own explanation.
    Failed(String),
}

impl fmt::Display for ActionError {
    /// Written to compose after a prefix, as error messages do:
    /// lowercase, no trailing punctuation.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("authorization was cancelled, so nothing was changed"),
            Self::NotAuthorized => f.write_str("authorization was refused, so nothing was changed"),
            Self::NoPolkit => f.write_str(
                "pkexec is not installed, so Keyloom cannot ask for administrator access \
                 (run the commands below in a terminal instead)",
            ),
            Self::Failed(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for ActionError {}

impl From<service::Error> for ActionError {
    fn from(err: service::Error) -> Self {
        Self::Failed(err.to_string())
    }
}

impl From<install::Error> for ActionError {
    fn from(err: install::Error) -> Self {
        Self::Failed(err.to_string())
    }
}

/// Run a program as the administrator through the desktop's polkit
/// prompt.
async fn privileged(program: &str, args: &[&str]) -> Result<(), ActionError> {
    let output = Command::new("pkexec")
        .arg(program)
        .args(args)
        .output()
        .await
        .map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => ActionError::NoPolkit,
            _ => ActionError::Failed(format!("could not run pkexec: {err}")),
        })?;
    classify_exit(
        output.status.code(),
        &String::from_utf8_lossy(&output.stderr),
    )
}

/// Interpret a `pkexec` exit: 126 is a dismissed prompt and 127 a
/// refusal (both pkexec's own), anything else the program's.
fn classify_exit(code: Option<i32>, stderr: &str) -> Result<(), ActionError> {
    let stderr = stderr.trim();
    match code {
        Some(0) => Ok(()),
        Some(126) => Err(ActionError::Cancelled),
        Some(127) if stderr.contains("Not authorized") => Err(ActionError::NotAuthorized),
        Some(code) => Err(ActionError::Failed(if stderr.is_empty() {
            format!("exited with status {code}")
        } else {
            stderr.to_owned()
        })),
        None => Err(ActionError::Failed("stopped by a signal".to_owned())),
    }
}

/// Add the user to the `input` group. Takes effect at the next login.
///
/// # Errors
///
/// When authorization is cancelled or refused, or `usermod` fails.
pub async fn add_to_input_group(user: &str) -> Result<(), ActionError> {
    privileged("usermod", &["-aG", INPUT_GROUP, user]).await
}

/// Install the udev rule, load the `uinput` module, and apply the rule
/// to the device ([`UINPUT_SCRIPT`]).
///
/// # Errors
///
/// When authorization is cancelled or refused, or a command fails.
pub async fn prepare_uinput() -> Result<(), ActionError> {
    privileged("/bin/sh", &["-c", UINPUT_SCRIPT]).await
}

/// Carry out the xremap step: download the release Keyloom ships into
/// the user's `~/.local/bin`, which is also how Keyloom's own download
/// is updated. A running unit of Keyloom's is restarted so the new
/// binary takes over. Nothing here needs the administrator.
///
/// # Errors
///
/// When the download, its verification, or the install fails, or when
/// `systemctl` refuses the restart.
pub async fn install_xremap(facts: &Facts) -> Result<(), ActionError> {
    install::download_and_install().await?;
    if matches!(
        facts.unit,
        UnitCheck::Keyloom { active: true, .. } | UnitCheck::Stale { active: true }
    ) {
        service::restart().await?;
    }
    Ok(())
}

/// Carry out the service step. Everything here runs as the user.
///
/// # Errors
///
/// When the unit file cannot be written or `systemctl` refuses.
pub async fn run_service_action(action: ServiceAction, facts: &Facts) -> Result<(), ActionError> {
    if action == ServiceAction::Start && matches!(facts.unit, UnitCheck::Foreign { .. }) {
        // Their unit, their file: only start it.
        return Ok(service::restart().await?);
    }
    save_service(facts).await?;
    service::enable().await?;
    // Without keyboard access the service cannot do anything yet; it
    // starts on its own at the next login instead of failing now.
    if facts.has_effective_access() {
        service::restart().await?;
    }
    Ok(())
}

/// Write the service file Keyloom would install now and have systemd
/// read it, leaving it neither enabled nor started. The service step's
/// fix goes on to do both; turning remapping on by hand starts here, so
/// the `systemctl` commands setup shows next have a unit to act on.
///
/// # Errors
///
/// When there is nothing to point the service at, the file cannot be
/// written, or `systemctl` refuses the reload.
pub async fn save_service(facts: &Facts) -> Result<(), ActionError> {
    let (Some(binary), Some(config)) = (facts.xremap_path(), facts.config.as_deref()) else {
        return Err(ActionError::Failed(
            "xremap must be installed before the service can be set up".to_owned(),
        ));
    };
    let text = service::unit_file(binary, config, facts.launch());
    let write_failed = |err: &dyn fmt::Display| {
        ActionError::Failed(format!("could not write the service file: {err}"))
    };
    // Writing the unit is ordinary blocking file work.
    tokio::task::spawn_blocking(move || service::install(&text))
        .await
        .map_err(|err| write_failed(&err))?
        .map_err(|err| write_failed(&err))?;
    service::daemon_reload().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::testing::TempDir;

    const STATUS: &str = "Name:\tkeyloom\nUid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\nGroups:\t4 27 995 1000 \n";
    const GROUPS: &str = "root:x:0:\ninput:x:995:blake, other\nsudo:x:27:blake\n";
    const PASSWD: &str =
        "root:x:0:0:root:/root:/bin/bash\nblake:x:1000:1000::/home/blake:/bin/zsh\n";

    /// A COSMIC session with a distribution's xremap that can ask it.
    fn facts() -> Facts {
        Facts {
            user: Some("blake".to_owned()),
            xremap: XremapCheck::Found {
                path: PathBuf::from("/usr/bin/xremap"),
                version: Some("0.15.13".to_owned()),
                desktops: Some(vec![Desktop::Cosmic]),
                managed: false,
            },
            group: GroupCheck::Effective,
            uinput: UinputCheck::Writable,
            unit: UnitCheck::Keyloom {
                active: true,
                enabled: true,
            },
            config: Some(PathBuf::from("/home/blake/.config/xremap/keyloom.yml")),
            session: Session {
                desktop: Some(Desktop::Cosmic),
                x11: false,
            },
        }
    }

    /// Keyloom's own download of the release it ships.
    fn managed(version: &str) -> XremapCheck {
        XremapCheck::Found {
            path: install::managed_path().expect("a home directory"),
            version: Some(version.to_owned()),
            desktops: Some(Desktop::ALL.to_vec()),
            managed: true,
        }
    }

    #[test]
    fn steps_know_their_order() {
        assert_eq!(Step::Xremap.index(), 0);
        assert_eq!(Step::Service.index(), 3);
        assert_eq!(Step::Xremap.previous(), None);
        assert_eq!(Step::Xremap.next(), Some(Step::InputGroup));
        assert_eq!(Step::Service.next(), None);
        assert_eq!(Step::Service.previous(), Some(Step::Uinput));
    }

    #[test]
    fn the_user_comes_from_the_status_and_passwd_files() {
        assert_eq!(real_uid(STATUS), Some(1000));
        assert_eq!(user_name(PASSWD, 1000).as_deref(), Some("blake"));
        assert_eq!(user_name(PASSWD, 1001), None);
        assert_eq!(real_uid("Name:\tx\n"), None);
        assert_eq!(current_user(STATUS, PASSWD).as_deref(), Some("blake"));
    }

    #[test]
    fn group_membership_distinguishes_effective_from_configured() {
        assert_eq!(
            group_check(GROUPS, STATUS, Some("blake")),
            GroupCheck::Effective
        );

        // Listed in the group, but this session predates it.
        let stale = STATUS.replace("Groups:\t4 27 995 1000 ", "Groups:\t4 27 1000 ");
        assert_eq!(
            group_check(GROUPS, &stale, Some("blake")),
            GroupCheck::NeedsLogin
        );
        assert_eq!(
            group_check(GROUPS, &stale, Some("other")),
            GroupCheck::NeedsLogin
        );
        assert_eq!(
            group_check(GROUPS, &stale, Some("nobody")),
            GroupCheck::NotMember
        );
        assert_eq!(group_check(GROUPS, &stale, None), GroupCheck::NotMember);

        // A primary group counts as in effect too.
        let primary = STATUS
            .replace("Gid:\t1000", "Gid:\t995")
            .replace("995 ", "");
        assert_eq!(group_check(GROUPS, &primary, None), GroupCheck::Effective);

        assert_eq!(
            group_check("root:x:0:\n", STATUS, Some("blake")),
            GroupCheck::NoGroup
        );
    }

    #[test]
    fn the_version_is_read_off_the_banner() {
        assert_eq!(
            parse_version("xremap 0.15.12\n").as_deref(),
            Some("0.15.12")
        );
        assert_eq!(parse_version("xremap\n"), None);
        assert_eq!(parse_version("something else"), None);
    }

    #[tokio::test]
    async fn the_binary_is_found_on_path() {
        let dir = TempDir::new("setup-path");
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let empty = dir.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        let path = std::env::join_paths([&empty, &bin]).unwrap();

        assert_eq!(find_on_path("xremap", Some(&path)).await, None);
        let binary = bin.join("xremap");
        fs::write(&binary, "#!/bin/sh\n").unwrap();
        assert_eq!(
            find_on_path("xremap", Some(&path)).await,
            None,
            "not executable"
        );
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(find_on_path("xremap", Some(&path)).await, Some(binary));
        assert_eq!(find_on_path("xremap", None).await, None);
    }

    #[tokio::test]
    async fn uinput_is_checked_by_opening_the_device() {
        let dir = TempDir::new("setup-uinput");
        let device = dir.path().join("uinput");
        assert_eq!(
            uinput_check_at(&device, false).await,
            UinputCheck::Missing {
                rule_installed: false
            }
        );
        fs::write(&device, "").unwrap();
        assert_eq!(uinput_check_at(&device, true).await, UinputCheck::Writable);
        fs::set_permissions(&device, fs::Permissions::from_mode(0o444)).unwrap();
        // Root can write anything; the check is only meaningful otherwise.
        if fs::OpenOptions::new().write(true).open(&device).is_err() {
            assert_eq!(
                uinput_check_at(&device, true).await,
                UinputCheck::NotWritable {
                    rule_installed: true
                }
            );
        }
        fs::set_permissions(&device, fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[tokio::test]
    async fn rules_files_count_when_they_mention_uinput() {
        assert!(rule_text_covers_uinput(RULE));
        assert!(rule_text_covers_uinput(
            "# comment\n\nKERNEL==\"uinput\", MODE=\"0660\"\n"
        ));
        assert!(!rule_text_covers_uinput("# KERNEL==\"uinput\"\n"));
        assert!(!rule_text_covers_uinput(
            "KERNEL==\"event*\", GROUP=\"input\"\n"
        ));
        let dir = TempDir::new("setup-rules");
        let path = dir.path().join(RULES_FILE);
        assert!(!rule_present(&path).await);
        fs::write(&path, format!("{RULE}\n")).unwrap();
        assert!(rule_present(&path).await);
    }

    #[test]
    fn the_uinput_script_matches_the_published_rule() {
        assert!(UINPUT_SCRIPT.contains(RULES_PATH));
        assert!(UINPUT_SCRIPT.contains("/etc/modules-load.d/uinput.conf"));
        assert!(UINPUT_SCRIPT.contains(&format!("'{RULE}'")));
        assert!(UINPUT_SCRIPT.starts_with("set -e\n"));
    }

    #[test]
    fn the_manual_uinput_commands_do_what_the_script_does() {
        let commands = uinput_commands();
        assert!(commands.starts_with(&format!(
            "echo 'KERNEL==\"uinput\", GROUP=\"input\", TAG+=\"uaccess\"' | sudo tee {RULES_PATH}\n"
        )));
        assert!(commands.contains("\necho uinput | sudo tee /etc/modules-load.d/uinput.conf\n"));
        // Every command the script runs, other than its shell options,
        // the files it writes (above), and waiting for udev, is run the
        // same way by hand.
        for line in UINPUT_SCRIPT.lines().map(str::trim) {
            if line.is_empty()
                || line.starts_with("set ")
                || line.starts_with("printf ")
                || line == "udevadm settle"
            {
                continue;
            }
            assert!(
                commands
                    .lines()
                    .any(|command| command == format!("sudo {line}")),
                "{line} is missing from the manual commands"
            );
        }
        assert!(
            commands.lines().all(|command| command.contains("sudo ")),
            "every command needs the administrator"
        );
    }

    #[test]
    fn exec_start_joins_continuation_lines() {
        let unit = "[Service]\nExecStart=/usr/bin/xremap \\\n  --watch \\\n  /home/me/xremap.yml\nRestart=always\n";
        assert_eq!(
            exec_start_of(unit),
            "/usr/bin/xremap --watch /home/me/xremap.yml"
        );
        assert_eq!(exec_start_of("[Unit]\nDescription=x\n"), "");
        assert_eq!(
            exec_start_of("ExecStart=/bin/a\nExecStart=\nExecStart=/bin/b\nExecStart=/bin/c\n"),
            "/bin/b\n/bin/c"
        );
    }

    #[test]
    fn the_supported_desktops_are_read_off_the_list() {
        assert_eq!(
            parse_desktops(
                "This variant of xremap supports: GNOME, KDE, Hypr, Niri, wlroots, COSMIC, Pantheon, X11, Socket\n"
            ),
            Some(Desktop::ALL.to_vec()),
            "every desktop, without the bridge"
        );
        assert_eq!(
            parse_desktops("This variant of xremap supports: COSMIC\n"),
            Some(vec![Desktop::Cosmic])
        );
        assert_eq!(
            parse_desktops("This variant of xremap supports: \n"),
            Some(Vec::new()),
            "a build with no desktop client at all"
        );
        assert_eq!(
            parse_desktops("error: unexpected argument '--list-desktops' found\n"),
            None,
            "an older xremap, or an answer in another shape"
        );
        assert_eq!(parse_desktops(""), None);
    }

    #[test]
    fn the_desktop_is_named_only_when_the_binary_can_ask_it() {
        let cosmic = Session {
            desktop: Some(Desktop::Cosmic),
            x11: false,
        };
        let full = facts().xremap;
        assert_eq!(
            launch_for(&full, cosmic),
            service::Launch {
                desktop: Some(Desktop::Cosmic),
                wait_for_wayland: true,
            }
        );

        let gnome_only = XremapCheck::Found {
            path: PathBuf::from("/usr/bin/xremap"),
            version: Some("0.15.13".to_owned()),
            desktops: Some(vec![Desktop::Gnome]),
            managed: false,
        };
        assert_eq!(
            launch_for(&gnome_only, cosmic),
            service::Launch::default(),
            "a build without this desktop's client is left to its own devices"
        );

        let old = XremapCheck::Found {
            path: PathBuf::from("/usr/bin/xremap"),
            version: Some("0.15.12".to_owned()),
            desktops: None,
            managed: false,
        };
        assert_eq!(
            launch_for(&old, cosmic),
            service::Launch::default(),
            "a binary without --list-desktops would refuse --desktop"
        );
        assert_eq!(
            launch_for(&XremapCheck::Missing, cosmic),
            service::Launch::default()
        );

        let x11 = Session {
            desktop: Some(Desktop::X11),
            x11: true,
        };
        assert_eq!(
            launch_for(&managed("0.15.13"), x11),
            service::Launch {
                desktop: Some(Desktop::X11),
                wait_for_wayland: false,
            },
            "no Wayland socket to wait for on X11"
        );
        assert_eq!(
            launch_for(&full, Session::default()),
            service::Launch::default(),
            "an unrecognized desktop is left to xremap"
        );
    }

    #[test]
    fn the_xremap_step_offers_a_download_or_an_update_for_keylooms_own_copy() {
        if install::asset().is_none() || install::managed_path().is_none() {
            // Nothing to offer on this processor, or without a home.
            return;
        }
        let mut facts = facts();
        assert_eq!(
            facts.xremap_action(),
            None,
            "a distribution's xremap is theirs"
        );

        facts.xremap = XremapCheck::Missing;
        assert_eq!(facts.xremap_action(), Some(XremapAction::Download));

        facts.xremap = managed(install::RELEASE);
        assert_eq!(
            facts.xremap_action(),
            None,
            "already the release Keyloom ships"
        );

        facts.xremap = managed("0.15.12");
        assert_eq!(facts.xremap_action(), Some(XremapAction::Update));

        // The user's own binary at Keyloom's location is never replaced.
        facts.xremap = XremapCheck::Found {
            path: install::managed_path().expect("a home directory"),
            version: Some("0.15.12".to_owned()),
            desktops: Some(Desktop::ALL.to_vec()),
            managed: false,
        };
        assert_eq!(facts.xremap_action(), None);
    }

    #[test]
    fn application_matching_follows_the_build_and_the_desktop() {
        let mut facts = facts();
        assert_eq!(
            facts.app_matching(),
            AppMatching::Supported(Desktop::Cosmic)
        );

        facts.session.desktop = Some(Desktop::Gnome);
        assert_eq!(
            facts.app_matching(),
            AppMatching::Unsupported {
                desktop: Desktop::Gnome,
                supports: vec![Desktop::Cosmic],
            }
        );

        facts.xremap = XremapCheck::Found {
            path: PathBuf::from("/usr/bin/xremap"),
            version: Some("0.15.12".to_owned()),
            desktops: None,
            managed: false,
        };
        assert_eq!(facts.app_matching(), AppMatching::Unreported);

        facts.session.desktop = None;
        assert_eq!(facts.app_matching(), AppMatching::UnknownDesktop);

        facts.xremap = XremapCheck::Missing;
        assert_eq!(facts.app_matching(), AppMatching::NotInstalled);
    }

    #[tokio::test]
    async fn the_binary_on_path_wins_over_keylooms_download() {
        let dir = TempDir::new("setup-locate");
        let bin = dir.path().join("bin");
        let local = dir.path().join(".local").join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&local).unwrap();
        let on_path = bin.join("xremap");
        let downloaded = local.join("xremap");
        let path = std::env::join_paths([&bin]).unwrap();

        assert_eq!(locate_in(Some(&path), Some(&downloaded)).await, None);
        fs::write(&downloaded, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&downloaded, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            locate_in(Some(&path), Some(&downloaded)).await,
            Some(downloaded.clone()),
            "the download is found even though its directory is not on PATH"
        );
        assert_eq!(
            locate_in(None, Some(&downloaded)).await,
            Some(downloaded.clone())
        );
        fs::write(&on_path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&on_path, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            locate_in(Some(&path), Some(&downloaded)).await,
            Some(on_path),
            "a copy the user has on PATH comes first"
        );
        assert_eq!(locate_in(None, None).await, None);
    }

    #[test]
    fn units_are_classified_by_marker_and_contents() {
        let config = Path::new("/home/me/.config/xremap/keyloom.yml");
        let ours = service::unit_file(
            Path::new("/usr/bin/xremap"),
            config,
            service::Launch::default(),
        );
        let classify = |active, enabled, text: Option<&str>| {
            classify_unit(
                active,
                enabled,
                Some(PathBuf::from(
                    "/home/me/.config/systemd/user/xremap.service",
                )),
                text,
                Some(&ours),
                Some(config),
            )
        };

        assert_eq!(
            classify(true, true, Some(&ours)),
            UnitCheck::Keyloom {
                active: true,
                enabled: true
            }
        );
        let moved = service::unit_file(
            Path::new("/usr/local/bin/xremap"),
            config,
            service::Launch::default(),
        );
        assert_eq!(
            classify(false, true, Some(&moved)),
            UnitCheck::Stale { active: false }
        );
        let named = service::unit_file(
            Path::new("/usr/bin/xremap"),
            config,
            service::Launch {
                desktop: Some(Desktop::Cosmic),
                wait_for_wayland: true,
            },
        );
        assert_eq!(
            classify(true, true, Some(&named)),
            UnitCheck::Stale { active: true },
            "a unit naming a desktop the binary no longer would is stale, and vice versa"
        );

        let theirs = "[Service]\nExecStart=/usr/bin/xremap --watch /home/me/other.yml\n";
        assert_eq!(
            classify(true, false, Some(theirs)),
            UnitCheck::Foreign {
                exec_start: "/usr/bin/xremap --watch /home/me/other.yml".to_owned(),
                reads_config: false,
                active: true,
                path: Some(PathBuf::from(
                    "/home/me/.config/systemd/user/xremap.service"
                )),
            }
        );
        let theirs_with_ours =
            "[Service]\nExecStart=/usr/bin/xremap %h/.config/xremap/keyloom.yml\n";
        assert!(matches!(
            classify(true, true, Some(theirs_with_ours)),
            UnitCheck::Foreign {
                reads_config: true,
                ..
            }
        ));
        assert!(matches!(
            classify(true, true, None),
            UnitCheck::Foreign {
                reads_config: false,
                ..
            }
        ));
    }

    #[test]
    fn only_the_startup_command_counts_as_reading_the_config() {
        let config = Path::new("/home/me/.config/xremap/keyloom.yml");
        let reads = |text: &str| {
            matches!(
                classify_unit(true, true, None, Some(text), None, Some(config)),
                UnitCheck::Foreign {
                    reads_config: true,
                    ..
                }
            )
        };

        assert!(
            !reads(
                "[Service]\n                 # Someday: /home/me/.config/xremap/keyloom.yml\n                 ExecStart=/usr/bin/xremap /home/me/mine.yml\n"
            ),
            "a path in a comment runs nothing"
        );
        assert!(
            !reads(
                "[Service]\n                 #ExecStart=/usr/bin/xremap %h/.config/xremap/keyloom.yml\n                 ExecStart=/usr/bin/xremap /home/me/mine.yml\n"
            ),
            "a commented-out ExecStart is not the command that runs"
        );
        assert!(
            !reads(
                "[Service]\n                 ExecStartPre=/usr/bin/test -f %h/.config/xremap/keyloom.yml\n                 ExecStart=/usr/bin/xremap /home/me/mine.yml\n"
            ),
            "a setting other than ExecStart does not load the config"
        );
        assert!(
            !reads(
                "[Service]\n                 ExecStart=/usr/bin/xremap %h/.config/xremap/keyloom.yml\n                 ExecStart=\n                 ExecStart=/usr/bin/xremap /home/me/mine.yml\n"
            ),
            "an empty ExecStart resets the list systemd runs"
        );

        assert!(
            reads(
                "[Service]\nExecStart=/usr/bin/xremap /home/me/mine.yml \\\n  %h/.config/xremap/keyloom.yml\n"
            ),
            "a continued command line still names the config"
        );
        assert!(
            reads(
                "[Service]\nExecStart=/usr/bin/xremap --watch /home/me/.config/xremap/keyloom.yml\n"
            ),
            "the absolute path Keyloom generates"
        );
    }

    #[tokio::test]
    async fn unit_check_maps_systemd_states() {
        let unit = |status| service::Unit {
            status,
            enabled: true,
            fragment_path: None,
        };
        assert_eq!(
            unit_check(unit(service::Status::Unavailable), None, None).await,
            UnitCheck::Unavailable
        );
        assert_eq!(
            unit_check(unit(service::Status::NotFound), None, None).await,
            UnitCheck::Missing
        );
        assert!(matches!(
            unit_check(unit(service::Status::Failed), None, None).await,
            UnitCheck::Foreign { active: false, .. }
        ));
    }

    #[test]
    fn verdicts_follow_the_facts() {
        let facts = facts();
        assert!(facts.is_all_ok());
        assert!(facts.is_configured());
        assert_eq!(facts.service_action(), None);

        let mut waiting = facts.clone();
        waiting.group = GroupCheck::NeedsLogin;
        waiting.uinput = UinputCheck::NotWritable {
            rule_installed: true,
        };
        waiting.unit = UnitCheck::Keyloom {
            active: false,
            enabled: true,
        };
        assert!(!waiting.is_all_ok());
        assert!(waiting.is_configured(), "only a login is missing");
        assert!(waiting.step_needs_login(Step::Service));
        assert_eq!(
            waiting.service_action(),
            None,
            "starting cannot help before the login"
        );

        // With the group already in effect, a login cannot make an
        // installed rule work: the step stays open for its fix.
        let mut ineffective = facts.clone();
        ineffective.uinput = UinputCheck::NotWritable {
            rule_installed: true,
        };
        assert!(!ineffective.step_needs_login(Step::Uinput));
        assert!(!ineffective.is_step_settled(Step::Uinput));
        ineffective.group = GroupCheck::NotMember;
        assert!(!ineffective.is_step_settled(Step::Uinput));

        let mut fresh = facts.clone();
        fresh.group = GroupCheck::NotMember;
        fresh.uinput = UinputCheck::NotWritable {
            rule_installed: false,
        };
        fresh.unit = UnitCheck::Missing;
        assert!(!fresh.is_configured());
        assert!(!fresh.step_needs_login(Step::Uinput));
        assert_eq!(fresh.service_action(), Some(ServiceAction::Install));
        assert!(
            fresh.can_save_service(),
            "the file can be saved to start by hand"
        );

        let mut no_xremap = fresh.clone();
        no_xremap.xremap = XremapCheck::Missing;
        assert_eq!(
            no_xremap.service_action(),
            None,
            "nothing to point a unit at"
        );
        assert!(!no_xremap.can_save_service());

        let mut paused = facts.clone();
        paused.unit = UnitCheck::Keyloom {
            active: false,
            enabled: true,
        };
        assert!(!paused.is_step_ok(Step::Service));
        assert_eq!(paused.service_action(), Some(ServiceAction::Start));

        let mut disabled = facts.clone();
        disabled.unit = UnitCheck::Keyloom {
            active: true,
            enabled: false,
        };
        assert!(!disabled.is_step_ok(Step::Service));
        assert_eq!(disabled.service_action(), Some(ServiceAction::Enable));

        let mut stale = facts.clone();
        stale.unit = UnitCheck::Stale { active: true };
        assert_eq!(stale.service_action(), Some(ServiceAction::Update));
        assert!(
            !stale.can_save_service(),
            "an existing service is updated, not saved beside"
        );

        let foreign = |reads_config, active| UnitCheck::Foreign {
            exec_start: String::new(),
            reads_config,
            active,
            path: None,
        };
        let mut theirs = facts.clone();
        theirs.unit = foreign(false, true);
        assert!(!theirs.is_step_ok(Step::Service));
        assert_eq!(theirs.service_action(), Some(ServiceAction::Replace));
        theirs.unit = foreign(true, true);
        assert!(theirs.is_step_ok(Step::Service));
        assert_eq!(theirs.service_action(), None);
        theirs.unit = foreign(true, false);
        assert_eq!(theirs.service_action(), Some(ServiceAction::Start));

        let mut no_systemd = facts;
        no_systemd.unit = UnitCheck::Unavailable;
        assert!(!no_systemd.is_configured());
        assert_eq!(no_systemd.service_action(), None);
    }

    #[test]
    fn setup_stops_only_where_something_is_left_to_do() {
        let ready = facts();
        for step in Step::ALL {
            assert!(!ready.wants_attention(step), "{step:?} is in order");
        }
        assert_eq!(ready.next_attention(None), None);

        // Joined and installed, waiting only for a login: passed over.
        let mut waiting = ready.clone();
        waiting.group = GroupCheck::NeedsLogin;
        waiting.uinput = UinputCheck::NotWritable {
            rule_installed: true,
        };
        waiting.unit = UnitCheck::Keyloom {
            active: false,
            enabled: true,
        };
        assert_eq!(waiting.next_attention(None), None);

        let mut fresh = ready.clone();
        fresh.group = GroupCheck::NotMember;
        fresh.unit = UnitCheck::Missing;
        assert_eq!(fresh.next_attention(None), Some(Step::InputGroup));
        assert_eq!(
            fresh.next_attention(Some(Step::InputGroup)),
            Some(Step::Service),
            "the virtual keyboard is already in order"
        );
        assert_eq!(fresh.next_attention(Some(Step::Service)), None);

        // A step Keyloom cannot fix still gets a stop, to explain itself.
        let mut blocked = ready.clone();
        blocked.unit = UnitCheck::Unavailable;
        assert_eq!(blocked.next_attention(None), Some(Step::Service));

        if install::asset().is_some() && install::managed_path().is_some() {
            // An update for Keyloom's own xremap is worth a stop too,
            // though the xremap it has works.
            let mut outdated = ready;
            outdated.xremap = managed("0.15.12");
            assert!(outdated.is_step_settled(Step::Xremap));
            assert_eq!(outdated.next_attention(None), Some(Step::Xremap));
            assert_eq!(outdated.next_attention(Some(Step::Xremap)), None);
        }
    }

    #[test]
    fn pkexec_exits_are_explained() {
        assert_eq!(classify_exit(Some(0), ""), Ok(()));
        assert_eq!(
            classify_exit(
                Some(126),
                "Error executing command as another user: Request dismissed"
            ),
            Err(ActionError::Cancelled)
        );
        assert_eq!(
            classify_exit(
                Some(127),
                "Error executing command as another user: Not authorized"
            ),
            Err(ActionError::NotAuthorized)
        );
        assert_eq!(
            classify_exit(Some(127), "sh: modprobe: not found"),
            Err(ActionError::Failed("sh: modprobe: not found".to_owned()))
        );
        assert_eq!(
            classify_exit(Some(1), ""),
            Err(ActionError::Failed("exited with status 1".to_owned()))
        );
        assert!(matches!(
            classify_exit(None, ""),
            Err(ActionError::Failed(_))
        ));
        assert!(!ActionError::NoPolkit.to_string().is_empty());
    }
}

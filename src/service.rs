//! Management of the xremap systemd *user* service.
//!
//! For now Keyloom assumes xremap is already installed and registered
//! as a user unit (see `docs/Functionality_TODO.md` §6 for the plan to
//! relax this): status comes from `systemctl --user show` and applying
//! a configuration means restarting the unit so xremap re-reads the
//! generated file. The header's status chip stops and starts the unit,
//! which is how a keyboard the remapper holds exclusively is released.

use tokio::process::Command;

/// The systemd user unit Keyloom manages.
pub const UNIT: &str = "xremap.service";

/// State of the xremap user service, as far as systemd knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The unit is loaded and running.
    Active,
    /// The unit exists but is not running.
    Inactive,
    /// The unit exists and its last run failed.
    Failed,
    /// No `xremap.service` user unit is registered.
    NotFound,
    /// `systemctl` is unavailable (not a systemd session).
    Unavailable,
}

impl Status {
    /// User-facing state label. xremap and systemd are implementation
    /// details, so the wording stays generic.
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Remapping Enabled",
            // A stopped unit is a paused one, however it came to be
            // stopped: the distinction is Keyloom's, not the user's.
            Self::Inactive => "Remapping Paused",
            Self::Failed => "Remapping Failed",
            Self::NotFound => "Remapping not set up",
            Self::Unavailable => "Remapping Unavailable",
        }
    }
}

/// Query the unit's state. `show` succeeds even for missing units,
/// reporting `LoadState=not-found`, which keeps the cases apart.
pub async fn status() -> Status {
    let output = Command::new("systemctl")
        .args(["--user", "show", UNIT, "--property=LoadState,ActiveState"])
        .output()
        .await;
    let Ok(output) = output else {
        return Status::Unavailable;
    };
    if !output.status.success() {
        return Status::Unavailable;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let property = |name: &str| {
        stdout
            .lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
    };
    if property("LoadState") == Some("not-found") {
        return Status::NotFound;
    }
    match property("ActiveState") {
        Some("active" | "activating" | "reloading") => Status::Active,
        Some("failed") => Status::Failed,
        _ => Status::Inactive,
    }
}

/// Restart the unit so xremap picks up the generated configuration.
pub async fn restart() -> Result<(), String> {
    run("restart").await
}

/// Stop the unit, releasing the keyboards xremap grabbed so they can
/// be observed directly. Only the status chip asks for this.
pub async fn stop() -> Result<(), String> {
    run("stop").await
}

/// Start the unit, whether or not this session is what stopped it.
pub async fn start() -> Result<(), String> {
    run("start").await
}

/// Run one `systemctl --user` verb against the unit. The error string
/// is meant for the failure toast.
async fn run(verb: &str) -> Result<(), String> {
    let output = Command::new("systemctl")
        .args(["--user", verb, UNIT])
        .output()
        .await
        .map_err(|err| format!("could not run systemctl: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        Err(format!("systemctl exited with {}", output.status))
    } else {
        Err(stderr.to_owned())
    }
}

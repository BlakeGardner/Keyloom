//! States of the window for tests to start from, built from values
//! rather than read off the machine the tests run on.
//!
//! A setup page is rendered from one value, [`Facts`], plus a few fields
//! of the open wizard ([`Setup`]), so a state is staged by building
//! those directly: nothing is read from `/etc`, `/dev`, or systemd, and
//! nothing is written. The interface tests (`e2e`) and the
//! screenshots (`screenshots`) share these builders, so what is
//! asserted on and what is pictured is one and the same state.

use std::path::PathBuf;

use cosmic::Application;
use cosmic::iced::core::Size;
use cosmic::widget;

use super::*;
use crate::install;
use crate::session::{Desktop, Session};
use crate::setup::{Facts, GroupCheck, Step, UinputCheck, UnitCheck, XremapCheck};

/// The window's default size, in logical pixels, so pages get exactly
/// the room they have at runtime, clipping included.
pub const WINDOW: Size = WINDOW_SIZE;

/// The smallest the window can be made.
pub const MIN_WINDOW: Size = MIN_WINDOW_SIZE;

/// The window as the runtime composes it: the header bar with the
/// application's own header widgets, the content beneath, and any
/// dialog centered over both (`libcosmic/src/app/mod.rs`).
pub fn window(app: &App) -> Element<'_, Message> {
    let mut header = widget::header_bar();
    for element in app.header_start() {
        header = header.start(element);
    }
    for element in app.header_center() {
        header = header.center(element);
    }
    for element in app.header_end() {
        header = header.end(element);
    }
    let column = widget::column::with_capacity(2)
        .push(header)
        .push(app.view());
    let mut popover = widget::popover(column).modal(true);
    if let Some(dialog) = app.dialog() {
        popover = popover.popup(dialog);
    }
    popover.into()
}

/// The log window as the runtime composes it ([`App::view_window`]):
/// it draws its own header.
pub fn log_window(app: &App) -> Element<'_, Message> {
    let log = app.log.as_ref().expect("the log window is open");
    app.view_window(log.id)
}

/// What the service's watcher delivers when the unit is in `status`:
/// a report newer than any before it.
pub fn service_is(status: service::Status) -> Message {
    Message::ServiceStatus(service::Snapshot::latest(status))
}

// ---- Systems -----------------------------------------------------------

pub const CONFIG: &str = "/home/blake/.config/xremap/keyloom.yml";
pub const UNIT_PATH: &str = "/home/blake/.config/systemd/user/xremap.service";

/// A system where everything is in order: the user's own xremap on a
/// COSMIC Wayland session, access granted, Keyloom's unit running.
pub fn ready() -> Facts {
    Facts {
        user: Some("blake".to_owned()),
        xremap: found(false, Some(install::RELEASE), Some(vec![Desktop::Cosmic])),
        group: GroupCheck::Effective,
        uinput: UinputCheck::Writable,
        unit: UnitCheck::Keyloom {
            active: true,
            enabled: true,
        },
        config: Some(PathBuf::from(CONFIG)),
        session: Session {
            desktop: Some(Desktop::Cosmic),
            x11: false,
        },
    }
}

/// [`ready`] with some facts changed.
pub fn facts(edit: impl FnOnce(&mut Facts)) -> Facts {
    let mut facts = ready();
    edit(&mut facts);
    facts
}

/// An xremap that was found: the user's own or Keyloom's download, at
/// some version, listing the desktops it can ask (or not).
pub fn found(managed: bool, version: Option<&str>, desktops: Option<Vec<Desktop>>) -> XremapCheck {
    XremapCheck::Found {
        path: PathBuf::from(if managed {
            "/home/blake/.local/bin/xremap"
        } else {
            "/usr/bin/xremap"
        }),
        version: version.map(str::to_owned),
        desktops,
        managed,
    }
}

/// A unit somebody else wrote, running or not, loading Keyloom's
/// config or their own.
pub fn foreign(reads_config: bool, active: bool) -> UnitCheck {
    let config = if reads_config {
        CONFIG
    } else {
        "/home/blake/.config/xremap/config.yml"
    };
    UnitCheck::Foreign {
        exec_start: format!("/usr/bin/xremap --watch {config}"),
        reads_config,
        active,
        path: Some(PathBuf::from(UNIT_PATH)),
    }
}

/// Everything setup has to do on a system with nothing: xremap missing,
/// no access, no service.
pub fn fresh_system() -> Facts {
    facts(|f| {
        f.xremap = XremapCheck::Missing;
        f.group = GroupCheck::NotMember;
        f.uinput = UinputCheck::Missing {
            rule_installed: false,
        };
        f.unit = UnitCheck::Missing;
    })
}

/// Systems that more than one test starts from, each [`ready`] but for
/// one thing.
pub mod systems {
    use super::*;

    pub fn xremap_missing() -> Facts {
        facts(|f| f.xremap = XremapCheck::Missing)
    }

    pub fn xremap_outdated() -> Facts {
        facts(|f| f.xremap = found(true, Some("0.15.10"), Some(vec![Desktop::Cosmic])))
    }

    pub fn not_member() -> Facts {
        facts(|f| f.group = GroupCheck::NotMember)
    }

    pub fn uinput_missing() -> Facts {
        facts(|f| {
            f.uinput = UinputCheck::Missing {
                rule_installed: false,
            }
        })
    }

    pub fn service_missing() -> Facts {
        facts(|f| f.unit = UnitCheck::Missing)
    }

    pub fn service_stale() -> Facts {
        facts(|f| f.unit = UnitCheck::Stale { active: true })
    }

    pub fn service_foreign_kept() -> Facts {
        facts(|f| f.unit = foreign(false, true))
    }

    pub fn service_foreign_stopped() -> Facts {
        facts(|f| f.unit = foreign(true, false))
    }

    /// Everything done except the login that makes access effective.
    pub fn waiting_for_login() -> Facts {
        facts(|f| {
            f.group = GroupCheck::NeedsLogin;
            f.uinput = UinputCheck::NotWritable {
                rule_installed: true,
            };
            f.unit = UnitCheck::Keyloom {
                active: false,
                enabled: true,
            };
        })
    }

    /// A home folder with a space in its name, and a unit of the user's
    /// own with a long command line: what the details have to wrap.
    pub fn long_paths() -> Facts {
        facts(|f| {
            f.config = Some(PathBuf::from("/home/Jo Doe/.config/xremap/keyloom.yml"));
            f.unit = UnitCheck::Foreign {
                exec_start: "/opt/xremap/bin/xremap --watch=config --device 'Keychron K2' \
                             --device 'ZSA Moonlander Mark I' --mouse \
                             /home/Jo Doe/.config/xremap/base.yml \
                             /home/Jo Doe/.config/xremap/work.yml"
                    .to_owned(),
                reads_config: false,
                active: true,
                path: Some(PathBuf::from(
                    "/home/Jo Doe/.config/systemd/user/xremap.service",
                )),
            };
        })
    }
}

// ---- The wizard --------------------------------------------------------

/// The application as tests start it: nothing read from or written to
/// the user's configuration, and setup closed.
pub fn app() -> App {
    App::init(Core::default(), ()).0
}

/// The wizard open on a page with the checks done (or, without facts,
/// still running).
pub fn app_on(page: SetupPage, facts: Option<Facts>) -> App {
    let mut app = app();
    app.setup = Some(Setup {
        page,
        probing: facts.is_none(),
        facts,
        ..Setup::new()
    });
    app
}

/// The wizard open on a step, with the open wizard's fields adjusted.
pub fn app_on_step(step: Step, facts: Facts, edit: impl FnOnce(&mut Setup)) -> App {
    let mut app = app_on(SetupPage::Step(step), Some(facts));
    edit(setup_of(&mut app));
    app
}

/// The open wizard, to adjust.
pub fn setup_of(app: &mut App) -> &mut Setup {
    app.setup.as_mut().expect("setup is open")
}

// ---- The remapping log -------------------------------------------------

/// The log window open, having been told `events`.
pub fn app_with_log(events: Vec<journal::Event>) -> App {
    let mut app = app();
    let _ = app.update(Message::OpenLog);
    for event in events {
        let _ = app.update(Message::LogEvent(event));
    }
    app
}

/// An entry of the log on October 2, 2026.
fn logged(source: journal::Source, run: &str, clock: &str, text: &str) -> journal::Entry {
    let mut parts = clock
        .split(':')
        .map(|part| part.parse::<i8>().expect("a clock"));
    let mut next = || parts.next().expect("hours, minutes, and seconds");
    let at = jiff::civil::date(2026, 10, 2).at(next(), next(), next(), 0);
    journal::Entry {
        at,
        clock: clock.to_owned(),
        text: text.to_owned(),
        writer: match source {
            journal::Source::Remapper => "xremap",
            journal::Source::Systemd => "systemd",
        }
        .to_owned(),
        source,
        priority: 6,
        run: Some(run.to_owned()),
    }
}

/// A log of two runs, the way journalctl reports one: a run that
/// started, watched the keyboards, and was restarted to apply a change,
/// then a run that failed on the configuration.
pub fn log_of_two_runs() -> Vec<journal::Entry> {
    use journal::Source::{Remapper, Systemd};
    const UNIT: &str = "xremap.service - Keyboard remapping for Keyloom (xremap)";
    const RULE: &str =
        "------------------------------------------------------------------------------";
    let a = |source, clock, text: &str| logged(source, "a", clock, text);
    let b = |source, clock, text: &str| logged(source, "b", clock, text);
    let mut entries = vec![
        a(Systemd, "20:58:15", &format!("Starting {UNIT}...")),
        a(Systemd, "20:58:15", &format!("Started {UNIT}.")),
        a(
            Remapper,
            "20:58:16",
            "Selecting devices from the following list:",
        ),
        a(Remapper, "20:58:16", RULE),
        a(Remapper, "20:58:16", "/dev/input/event0 : Power Button"),
        a(
            Remapper,
            "20:58:16",
            "/dev/input/event4 : TESmart DKS202-P24",
        ),
        a(Remapper, "20:58:16", "/dev/input/event6 : @HFD NEO80"),
        a(Remapper, "20:58:16", RULE),
        a(
            Remapper,
            "20:58:16",
            "Selected keyboards automatically since --device options weren't specified:",
        ),
        a(Remapper, "20:58:16", "/dev/input/event6 : @HFD NEO80"),
        a(
            Remapper,
            "21:07:12",
            "application-client: COSMIC (supported: true)",
        ),
        a(Remapper, "21:07:40", "application: google-chrome"),
        a(
            Remapper,
            "21:30:02",
            "Failed to ungrab device: No such device (os error 19)",
        ),
        a(Systemd, "21:30:05", &format!("Stopping {UNIT}...")),
        a(Systemd, "21:30:05", &format!("Stopped {UNIT}.")),
        b(Systemd, "21:30:05", &format!("Starting {UNIT}...")),
        b(Systemd, "21:30:06", &format!("Started {UNIT}.")),
        b(
            Remapper,
            "21:30:06",
            "Error: Failed to load config: unknown key `remapp` at line 4 column 5",
        ),
        b(
            Systemd,
            "21:30:06",
            "xremap.service: Main process exited, code=exited, status=1/FAILURE",
        ),
        b(
            Systemd,
            "21:30:06",
            "xremap.service: Failed with result 'exit-code'.",
        ),
    ];
    // systemd says how a run ended at notice and warning priority.
    let len = entries.len();
    entries[len - 2].priority = 5;
    entries[len - 1].priority = 4;
    entries
}

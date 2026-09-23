//! States of the window for tests to start from, built from values
//! rather than read off the machine the tests run on.
//!
//! A setup page is rendered from one value, [`Facts`], plus a few fields
//! of the open wizard ([`Setup`]), so a state is staged by building
//! those directly: nothing is read from `/etc`, `/dev`, or `systemctl`,
//! and nothing is written. The interface tests (`e2e`) and the
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

/// The window's default size (`src/main.rs`), in logical pixels, so
/// pages get exactly the room they have at runtime, clipping included.
pub const WINDOW: Size = Size::new(1210.0, 620.0);

/// The smallest the window can be made (`src/main.rs`).
pub const MIN_WINDOW: Size = Size::new(760.0, 480.0);

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

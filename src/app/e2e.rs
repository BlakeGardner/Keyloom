//! End-to-end tests of the interface: what the window shows, and what
//! pressing its controls does, checked the way a user sees it.
//!
//! Each test stages a state (`staging`), builds the window as the
//! runtime composes it, and drives it through `iced_test`: widgets are
//! found by the text they show and clicked, and the messages they emit
//! go through [`App::update`] as they would at runtime. A regression in
//! a page's wording, in which button a page offers, or in what a button
//! does fails the build here instead of waiting to be noticed in a
//! screenshot.
//!
//! Nothing on the machine is read or changed. A fix never runs: the
//! task `update` returns for it is dropped, and its outcome, and the
//! checks that follow it, are delivered as the runtime would deliver
//! them ([`Driver::deliver`]). The renderer is iced's software one,
//! chosen by `ICED_TEST_BACKEND` in `.cargo/config.toml`, so the layout
//! is the same on a workstation and on a CI runner without a GPU.
//!
//! The states and flows are those of `docs/Setup_Test_Matrix.md`, under
//! the identifiers used there; the screenshot tests (`screenshots`)
//! picture the same tables and flows. Prose with a link in it is rich
//! text, which cannot be found by its content, so a note that carries a
//! link is checked in a screenshot rather than here.
//!
//! A failure's message lists every text the window shows. For what a
//! list cannot tell, such as a control hidden behind another, a test
//! that fails also leaves a picture of the window as it was under
//! `target/setup-shots/failures/`, which CI uploads with the failed run.

use std::cell::Cell;

use cosmic::Application;
use cosmic::iced::core::{Pixels, Settings, Size};
use iced_test::Simulator;
use iced_test::selector::{Candidate, Target};

use super::screenshots::Shots;
use super::staging::{
    MIN_WINDOW, WINDOW, app, app_on, app_on_step, facts, foreign, found, fresh_system, ready,
    systems, window,
};
use super::*;
use crate::install;
use crate::session::Desktop;
use crate::setup::{ActionError, Facts, GroupCheck, Step, UinputCheck, UnitCheck, XremapCheck};

/// What a fix that asks for a password shows while the prompt is up.
const APPROVAL: &str = "Waiting for approval…";

thread_local! {
    /// Whether the test on this thread has had its failure pictured. A
    /// test that made several drivers drops them all while it unwinds,
    /// the one it was driving first; only that one is worth a picture.
    static PICTURED: Cell<bool> = const { Cell::new(false) };
}

// ---- Driving the window ------------------------------------------------

/// Where the frames of a flow go while a storyboard is recorded: the
/// screenshot tests pass a recorder, the tests here pass `None`.
pub type Recorder = Option<Box<dyn FnMut(&App, &str)>>;

/// The application, driven through its interface.
pub struct Driver {
    pub app: App,
    /// The window's size; pages taller than it scroll.
    viewport: Size,
    /// Names the failure, where a test drives more than one state.
    name: String,
    recorder: Recorder,
}

impl Driver {
    /// Drive `app` in a window of the default size.
    pub fn new(app: App) -> Self {
        Self::with_recorder(app, None)
    }

    /// [`new`](Self::new), with the frames of the flow going to `recorder`.
    pub fn with_recorder(app: App, recorder: Recorder) -> Self {
        Self {
            app,
            viewport: WINDOW,
            name: String::new(),
            recorder,
        }
    }

    /// Name the state being driven, for failures to point at.
    pub fn named(mut self, name: &str) -> Self {
        self.name = format!("{name}: ");
        self
    }

    /// Shrink the window to the smallest size it can be given.
    pub fn at_minimum_size(mut self) -> Self {
        self.viewport = MIN_WINDOW;
        self
    }

    /// A frame of the flow, for a storyboard being recorded.
    pub fn frame(&mut self, label: &str) {
        if let Some(record) = &mut self.recorder {
            record(&self.app, label);
        }
    }

    /// The window as it is now, laid out to be inspected and clicked.
    fn simulator(&self) -> Simulator<'_, Message, cosmic::Theme, cosmic::Renderer> {
        let settings = Settings {
            default_font: cosmic::font::default(),
            default_text_size: Pixels(14.0),
            ..Settings::default()
        };
        Simulator::with_size(settings, self.viewport, window(&self.app))
    }

    /// Every text the window shows, in the order the widgets come.
    pub fn texts(&self) -> Vec<String> {
        let mut texts = Vec::new();
        // A selector that matches nothing walks the whole tree.
        let _ = self
            .simulator()
            .find(|candidate: Candidate<'_>| -> Option<()> {
                if let Candidate::Text { content, .. } = candidate {
                    texts.push(content.to_owned());
                }
                None
            });
        texts
    }

    /// Whether the window shows exactly this text somewhere.
    pub fn shows(&self, text: &str) -> bool {
        self.simulator().find(text).is_ok()
    }

    /// Fail unless the window shows each of these texts, exactly.
    #[track_caller]
    pub fn expect_shown(&self, texts: &[&str]) {
        let shown = self.texts();
        for text in texts {
            assert!(
                shown.iter().any(|s| s == text),
                "{}expected {text:?} on the window, which shows {shown:#?}",
                self.name
            );
        }
    }

    /// Fail unless each of these is part of some text on the window.
    #[track_caller]
    pub fn expect_parts(&self, parts: &[&str]) {
        let shown = self.texts();
        for part in parts {
            assert!(
                shown.iter().any(|s| s.contains(part)),
                "{}expected {part:?} within a text on the window, which shows {shown:#?}",
                self.name
            );
        }
    }

    /// Fail if the window shows this text.
    #[track_caller]
    pub fn expect_hidden(&self, text: &str) {
        assert!(
            !self.shows(text),
            "{}expected {text:?} to be gone from the window, which shows {:#?}",
            self.name,
            self.texts()
        );
    }

    /// Click the last widget showing `text`, and give the messages it
    /// emits to `update`; a page whose title says the same as its main
    /// button gets its button pressed. Returns how many messages there
    /// were: a disabled button emits none. Fails when nothing shows the
    /// text, or when it is scrolled out of view: what a user cannot
    /// reach, a test cannot press either.
    #[track_caller]
    pub fn click(&mut self, text: &str) -> usize {
        let messages: Vec<Message> = {
            let mut ui = self.simulator();
            let mut targets = Vec::new();
            let _ = ui.find(|candidate: Candidate<'_>| -> Option<()> {
                if matches!(&candidate, Candidate::Text { content, .. } if *content == text) {
                    targets.push(Target::from(candidate));
                }
                None
            });
            let Some(target) = targets.pop() else {
                panic!(
                    "{}nothing to click shows {text:?}; the window shows {:#?}",
                    self.name,
                    self.texts()
                );
            };
            let Some(bounds) = target.visible_bounds() else {
                panic!(
                    "{}{text:?} is out of view, so it cannot be clicked",
                    self.name
                );
            };
            ui.point_at(bounds.center());
            let _ = ui.simulate(iced_test::simulator::click());
            ui.into_messages().collect()
        };
        let count = messages.len();
        for message in messages {
            self.deliver(message);
        }
        count
    }

    /// Give the application a message the runtime would deliver on its
    /// own: what a control emitted, the outcome of a fix, or what the
    /// checks found.
    pub fn deliver(&mut self, message: Message) {
        // The task is what the fix or the checks would do to the
        // system; it is never run.
        let _task = self.app.update(message);
    }

    /// A fix that worked, and the checks agreeing afterwards.
    pub fn fixed(&mut self, step: Step, after: Facts) {
        self.deliver(Message::SetupActed {
            step,
            result: Ok(()),
        });
        self.deliver(Message::SetupProbed(after));
    }

    /// A fix that failed, and the checks finding the system as it was.
    pub fn failed(&mut self, step: Step, error: ActionError, unchanged: Facts) {
        self.deliver(Message::SetupActed {
            step,
            result: Err(error),
        });
        self.deliver(Message::SetupProbed(unchanged));
    }

    /// The open wizard.
    #[track_caller]
    pub fn setup(&self) -> &Setup {
        self.app.setup.as_ref().expect("setup is open")
    }

    /// The page the wizard is showing.
    #[track_caller]
    pub fn page(&self) -> SetupPage {
        self.setup().page
    }

    /// Picture the window as the failing test left it, named after the
    /// test, for reading beside the failure's message.
    fn picture_failure(&self) {
        if PICTURED.replace(true) {
            return;
        }
        let test = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .replace("::", "-");
        let name = format!("failures/{test}");
        // A panic while the test unwinds would abort the whole test run,
        // so rendering must not be allowed to raise one.
        let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Shots::new().capture_at(&self.app, &name, self.viewport);
        }));
        if rendered.is_err() {
            eprintln!("failure screenshot: could not render {name}");
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.picture_failure();
        }
    }
}

// ---- The rows of the matrix --------------------------------------------

/// The main button of a step page, and what pressing it does.
#[derive(Clone, Copy, Debug)]
pub enum Main {
    /// Nothing to fix: "Continue" moves on to the next step that wants
    /// attention, or to the summary.
    Continue,
    /// A fix, by its label and what the button says while the fix runs.
    Fix(&'static str, &'static str),
    /// "Check again": the user does the step by hand first.
    Recheck,
    /// Another step has to come first, by the label of the button that
    /// leads there.
    Visit(Step, &'static str),
}

/// One row of a step's table in the matrix: the system it stages, and
/// what the page shows for it.
pub struct Row {
    pub id: &'static str,
    pub facts: Facts,
    /// The status line.
    pub status: &'static str,
    pub title: &'static str,
    pub main: Main,
    /// Parts of what the page shows once its details are open: what the
    /// step found, the commands for doing it by hand, and the controls
    /// beside them ("Copy" exactly where there are commands to copy).
    pub shown: &'static [&'static str],
}

fn row(
    id: &'static str,
    facts: Facts,
    status: &'static str,
    title: &'static str,
    main: Main,
    shown: &'static [&'static str],
) -> Row {
    Row {
        id,
        facts,
        status,
        title,
        main,
        shown,
    }
}

/// Step 1: X1–X6 and XA2–XA6. Whether a download is offered (X3, X5)
/// or the user is sent to the project page (X6) depends on whether this
/// build has an xremap download for its processor, as at runtime.
pub fn xremap_rows() -> Vec<Row> {
    let installed = |id, facts, shown| {
        row(
            id,
            facts,
            "Installed",
            "xremap is installed",
            Main::Continue,
            shown,
        )
    };
    let gnome = |f: &mut Facts| {
        f.xremap = found(false, Some(install::RELEASE), Some(vec![Desktop::Gnome]));
        f.session.desktop = Some(Desktop::Gnome);
    };
    let has_download = install::asset().is_some();

    let mut rows = vec![
        installed(
            "X1-installed",
            ready(),
            &[
                "Binary: /usr/bin/xremap",
                "Desktop: COSMIC (Wayland session)",
                "Can ask: COSMIC",
                "This build can tell which window is in front on COSMIC",
            ],
        ),
        installed(
            "X2-installed-keyloom-download",
            facts(|f| f.xremap = found(true, Some(install::RELEASE), Some(vec![Desktop::Cosmic]))),
            &[
                "Binary: /home/blake/.local/bin/xremap",
                "(installed by Keyloom)",
            ],
        ),
    ];
    rows.push(if has_download {
        row(
            "X3-update-available",
            systems::xremap_outdated(),
            "Update available",
            "Update xremap",
            Main::Fix("Update xremap", "Updating…"),
            &[
                "Keyloom only ever updates the copy it installed itself",
                "Version: 0.15.10 (installed by Keyloom)",
                "Download: https://github.com/xremap/xremap/releases/download/",
            ],
        )
    } else {
        installed(
            "X3-update-not-available",
            systems::xremap_outdated(),
            &["Version: 0.15.10 (installed by Keyloom)"],
        )
    });
    rows.push(installed(
        "X4-version-unknown",
        facts(|f| f.xremap = found(false, None, Some(vec![Desktop::Cosmic]))),
        &[
            "Binary: /usr/bin/xremap",
            "Desktop: COSMIC (Wayland session)",
        ],
    ));
    rows.push(if has_download {
        row(
            "X5-not-installed",
            systems::xremap_missing(),
            "Not installed",
            "Install xremap",
            Main::Fix("Install xremap", "Installing…"),
            &[
                "Download: https://github.com/xremap/xremap/releases/download/",
                "SHA-256: ",
                "Installs to: ",
            ],
        )
    } else {
        row(
            "X6-no-download",
            systems::xremap_missing(),
            "Not installed",
            "Install xremap",
            Main::Recheck,
            &["Keyloom looked for xremap on your PATH and in ~/.local/bin."],
        )
    });
    rows.extend([
        installed(
            "XA2-gnome-wayland",
            facts(gnome),
            &["Desktop: GNOME (Wayland session)", "Can ask: GNOME"],
        ),
        installed(
            "XA3-gnome-x11",
            facts(|f| {
                gnome(f);
                f.session.x11 = true;
            }),
            &["Desktop: GNOME (X11 session)"],
        ),
        installed(
            "XA4-unsupported-desktop",
            facts(|f| {
                f.xremap = found(
                    false,
                    Some(install::RELEASE),
                    Some(vec![Desktop::Gnome, Desktop::Kde]),
                );
            }),
            &[
                "This xremap can't tell which application is in front on COSMIC, so \
                 application-specific remaps won't work here.",
                "Can ask: GNOME, KDE Plasma",
            ],
        ),
        installed(
            "XA5-unreported",
            facts(|f| f.xremap = found(false, Some("0.10.0"), None)),
            &["This xremap doesn't list the desktops it can ask"],
        ),
        installed(
            "XA6-unknown-desktop",
            facts(|f| f.session.desktop = None),
            &[
                "Desktop: not recognized (Wayland session)",
                "Keyloom couldn't tell which desktop this is",
            ],
        ),
    ]);
    rows
}

/// Step 2: G1–G5.
pub fn keyboard_access_rows() -> Vec<Row> {
    vec![
        row(
            "G1-allowed",
            ready(),
            "Allowed",
            "Keyboard access is allowed",
            Main::Continue,
            &["You're in the input group, and it's in effect for this session."],
        ),
        row(
            "G2-next-login",
            facts(|f| f.group = GroupCheck::NeedsLogin),
            "Takes effect after a restart",
            "Keyboard access is set up",
            Main::Continue,
            &["this session started before you joined it"],
        ),
        row(
            "G3-not-allowed",
            systems::not_member(),
            "Not allowed yet",
            "Allow keyboard access",
            Main::Fix("Allow keyboard access", APPROVAL),
            &["sudo usermod -aG input blake", "Copy"],
        ),
        row(
            "G4-not-allowed-user-unknown",
            facts(|f| {
                f.group = GroupCheck::NotMember;
                f.user = None;
            }),
            "Not allowed yet",
            "Allow keyboard access",
            Main::Recheck,
            &["sudo usermod -aG input $USER", "Copy"],
        ),
        row(
            "G5-no-group",
            facts(|f| f.group = GroupCheck::NoGroup),
            "No input group",
            "Keyboard access isn't available",
            Main::Continue,
            &["This system has no input group"],
        ),
    ]
}

/// Step 3: U1–U6.
pub fn virtual_keyboard_rows() -> Vec<Row> {
    let not_writable = |rule_installed: bool| UinputCheck::NotWritable { rule_installed };
    let fix = Main::Fix("Allow virtual keyboard", APPROVAL);
    vec![
        row(
            "U1-allowed",
            ready(),
            "Allowed",
            "Virtual keyboard is allowed",
            Main::Continue,
            &["This session can use /dev/uinput."],
        ),
        row(
            "U2-missing",
            systems::uinput_missing(),
            "Not set up",
            "Allow the virtual keyboard",
            fix,
            &[
                "The uinput module isn't loaded",
                "sudo modprobe uinput",
                "Copy",
            ],
        ),
        row(
            "U3-missing-rule-installed",
            facts(|f| {
                f.uinput = UinputCheck::Missing {
                    rule_installed: true,
                }
            }),
            "Not set up",
            "Allow the virtual keyboard",
            fix,
            &[
                "The uinput module isn't loaded",
                "sudo modprobe uinput",
                "Copy",
            ],
        ),
        row(
            "U4-not-allowed",
            facts(|f| f.uinput = not_writable(false)),
            "Not allowed yet",
            "Allow the virtual keyboard",
            fix,
            &[
                "Only the administrator can use /dev/uinput right now.",
                "sudo modprobe uinput",
                "Copy",
            ],
        ),
        row(
            "U5-next-login",
            facts(|f| {
                f.uinput = not_writable(true);
                f.group = GroupCheck::NeedsLogin;
            }),
            "Takes effect after a restart",
            "Virtual keyboard is set up",
            Main::Continue,
            &["The rule is installed (/etc/udev/rules.d/00-xremap-input.rules)"],
        ),
        row(
            "U6-not-in-effect",
            facts(|f| f.uinput = not_writable(true)),
            "Not in effect",
            "Allow the virtual keyboard",
            Main::Fix("Set up again", APPROVAL),
            &[
                "A rule for /dev/uinput is installed, but this session can't write to it.",
                "Copy",
            ],
        ),
    ]
}

/// Step 4: S1–S18.
pub fn service_rows() -> Vec<Row> {
    let keyloom = |active: bool, enabled: bool| UnitCheck::Keyloom { active, enabled };
    let set_up = |id, facts, status| {
        row(
            id,
            facts,
            status,
            "Remapping is set up",
            Main::Continue,
            &["Remaps: /home/blake/.config/xremap/keyloom.yml"],
        )
    };
    let replace = |id, facts, shown| {
        row(
            id,
            facts,
            "Doesn't use Keyloom's remaps",
            "Replace your xremap service?",
            Main::Fix("Replace service", "Replacing…"),
            shown,
        )
    };
    let stale = |id, facts| {
        row(
            id,
            facts,
            "Needs an update",
            "Update remapping",
            Main::Fix("Update remapping", "Updating…"),
            &["Keyloom rewrites its service at"],
        )
    };
    let cannot = |id, facts, status, shown| {
        row(
            id,
            facts,
            status,
            "Remapping can't be turned on here",
            Main::Continue,
            shown,
        )
    };
    vec![
        row(
            "S1-running",
            ready(),
            "Running",
            "Remapping is on",
            Main::Continue,
            &["Remaps: /home/blake/.config/xremap/keyloom.yml"],
        ),
        set_up(
            "S2-set-up-starts-at-login",
            systems::waiting_for_login(),
            "Starts after a restart",
        ),
        set_up(
            "S3-set-up-waiting-for-access",
            facts(|f| {
                f.uinput = UinputCheck::NotWritable {
                    rule_installed: false,
                };
                f.unit = keyloom(false, true);
            }),
            "Waiting for keyboard access",
        ),
        row(
            "S4-not-running",
            facts(|f| f.unit = keyloom(false, true)),
            "Not running",
            "Turn remapping back on",
            Main::Fix("Start remapping", "Starting…"),
            &["systemctl --user start xremap.service", "Copy"],
        ),
        row(
            "S5-not-at-login",
            facts(|f| f.unit = keyloom(true, false)),
            "Doesn't start at login",
            "Keep remapping on",
            Main::Fix("Start at login", "Working…"),
            &["systemctl --user enable xremap.service", "Copy"],
        ),
        row(
            "S6-turned-off",
            facts(|f| f.unit = keyloom(false, false)),
            "Turned off",
            "Turn on remapping",
            Main::Fix("Turn on remapping", "Turning on…"),
            &["systemctl --user enable --now xremap.service", "Copy"],
        ),
        stale("S7-stale-running", systems::service_stale()),
        stale(
            "S8-stale-stopped",
            facts(|f| f.unit = UnitCheck::Stale { active: false }),
        ),
        row(
            "S9-foreign-works",
            facts(|f| f.unit = foreign(true, true)),
            "Running (your own service)",
            "Your xremap service works with Keyloom",
            Main::Continue,
            &["ExecStart=/usr/bin/xremap --watch /home/blake/.config/xremap/keyloom.yml"],
        ),
        row(
            "S10-foreign-stopped",
            systems::service_foreign_stopped(),
            "Not running (your own service)",
            "Start your xremap service",
            Main::Fix("Start service", "Starting…"),
            &["systemctl --user start xremap.service", "Copy"],
        ),
        replace(
            "S11-foreign-replace",
            systems::service_foreign_kept(),
            &[
                "ExecStart=/usr/bin/xremap --watch /home/blake/.config/xremap/config.yml",
                "Keep mine",
            ],
        ),
        replace(
            "S12-foreign-replace-stopped",
            facts(|f| f.unit = foreign(false, false)),
            &[
                "ExecStart=/usr/bin/xremap --watch /home/blake/.config/xremap/config.yml",
                "Keep mine",
            ],
        ),
        replace(
            "S13-foreign-unreadable",
            facts(|f| {
                f.unit = UnitCheck::Foreign {
                    exec_start: String::new(),
                    reads_config: false,
                    active: true,
                    path: Some(PathBuf::from(super::staging::UNIT_PATH)),
                }
            }),
            &["ExecStart=(could not be read)", "Keep mine"],
        ),
        row(
            "S14-not-set-up",
            systems::service_missing(),
            "Not set up",
            "Turn on remapping",
            Main::Fix("Turn on remapping", "Turning on…"),
            &["save the file first", "Save service file"],
        ),
        row(
            "S15-needs-xremap",
            facts(|f| {
                f.xremap = XremapCheck::Missing;
                f.unit = UnitCheck::Missing;
            }),
            "Needs xremap",
            "Turn on remapping",
            Main::Visit(Step::Xremap, "Back to xremap"),
            &["once xremap is installed"],
        ),
        cannot(
            "S16-no-home",
            facts(|f| {
                f.config = None;
                f.unit = UnitCheck::Missing;
            }),
            "No home folder",
            &["Keyloom keeps your remaps under $XDG_CONFIG_HOME"],
        ),
        cannot(
            "S17-no-systemd",
            facts(|f| f.unit = UnitCheck::Unavailable),
            "No systemd user session",
            &[
                "/usr/bin/xremap --desktop cosmic --watch /home/blake/.config/xremap/keyloom.yml",
                "Copy",
            ],
        ),
        cannot(
            "S18-no-systemd-nothing-known",
            facts(|f| {
                f.xremap = XremapCheck::Missing;
                f.config = None;
                f.unit = UnitCheck::Unavailable;
            }),
            "No systemd user session",
            &["Once xremap is installed, you can run it on Keyloom's remaps yourself."],
        ),
    ]
}

/// Every row of a step's table: the page shows the row's status, title,
/// and main button; its details show what the row expects, with "Copy"
/// exactly where there are commands to copy; and the main button does
/// what it says, holding the other controls while a fix runs (T1).
fn check_step(step: Step, rows: Vec<Row>) {
    let eyebrow = format!(
        "FIRST-RUN SETUP · STEP {} OF {}",
        step.index() + 1,
        Step::ALL.len()
    );
    for row in rows {
        let id = row.id;
        let next = row
            .facts
            .next_attention(Some(step))
            .map_or(SetupPage::Finish, SetupPage::Step);
        let mut driver = Driver::new(app_on(SetupPage::Step(step), Some(row.facts))).named(id);
        driver.expect_shown(&[eyebrow.as_str(), row.status, row.title]);
        driver.expect_hidden("Hide details");

        driver.click("Show details");
        driver.expect_shown(&["Hide details"]);
        driver.expect_parts(row.shown);
        if !row.shown.contains(&"Copy") {
            driver.expect_hidden("Copy");
        }

        match row.main {
            Main::Continue => {
                driver.expect_shown(&["Continue"]);
                driver.click("Continue");
                assert_eq!(driver.page(), next, "{id}: Continue moves on");
            }
            Main::Fix(label, working) => {
                driver.expect_shown(&[label]);
                driver.expect_hidden(working);
                driver.click(label);
                assert_eq!(driver.setup().busy, Some(step), "{id}: the fix starts");
                driver.expect_shown(&[working]);
                assert_eq!(driver.click(working), 0, "{id}: one fix at a time");
                for held in [
                    "Check again",
                    "Skip this step",
                    "Keep mine",
                    "Save service file",
                ] {
                    if driver.shows(held) {
                        assert_eq!(driver.click(held), 0, "{id}: {held} waits for the fix");
                    }
                }
                assert_eq!(driver.page(), SetupPage::Step(step), "{id}: the page stays");
            }
            Main::Recheck => {
                driver.expect_shown(&["Check again"]);
                driver.click("Check again");
                assert!(driver.setup().probing, "{id}: the checks run again");
                driver.expect_shown(&["Checking…"]);
                driver.expect_hidden("Check again");
            }
            Main::Visit(target, label) => {
                driver.expect_shown(&[label]);
                driver.click(label);
                assert_eq!(
                    driver.page(),
                    SetupPage::Step(target),
                    "{id}: leads to the step"
                );
            }
        }
    }
}

#[test]
fn xremap_step_pages() {
    check_step(Step::Xremap, xremap_rows());
}

#[test]
fn keyboard_access_step_pages() {
    check_step(Step::InputGroup, keyboard_access_rows());
}

#[test]
fn virtual_keyboard_step_pages() {
    check_step(Step::Uinput, virtual_keyboard_rows());
}

#[test]
fn service_step_pages() {
    check_step(Step::Service, service_rows());
}

#[test]
fn xremap_details_show_the_version_only_when_it_is_known() {
    let version = format!("Version: {}", install::RELEASE);
    let known = Driver::new(app_on_step(Step::Xremap, ready(), |s| s.details = true)).named("X1");
    known.expect_parts(&[&version]);

    let unknown = Driver::new(app_on_step(
        Step::Xremap,
        facts(|f| f.xremap = found(false, None, Some(vec![Desktop::Cosmic]))),
        |s| s.details = true,
    ))
    .named("X4-version-unknown");
    assert!(
        !unknown.texts().iter().any(|text| text.contains("Version")),
        "an unreadable version is left out rather than shown blank"
    );
}

// ---- The pages around the steps ----------------------------------------

#[test]
fn welcome_page_starts_setup_or_puts_it_off() {
    // W1: the checks are still running; the first step waits for them.
    let mut driver = Driver::new(app_on(SetupPage::Welcome, None)).named("W1-checking");
    driver.expect_shown(&[
        "FIRST-RUN SETUP",
        "Make your keyboard your own",
        "Set up later",
        "Start setup",
    ]);
    driver.click("Start setup");
    assert_eq!(driver.page(), SetupPage::Step(Step::Xremap));
    driver.expect_shown(&["FIRST-RUN SETUP · STEP 1 OF 4", "xremap", "Checking…"]);
    driver.expect_hidden("Show details");
    driver.deliver(Message::SetupProbed(systems::xremap_missing()));
    driver.expect_shown(&["Not installed", "Install xremap", "Show details"]);

    // W2, with nothing to do: straight to the summary.
    let mut driver = Driver::new(app_on(SetupPage::Welcome, Some(ready()))).named("W2");
    driver.click("Start setup");
    assert_eq!(driver.page(), SetupPage::Finish);
    driver.expect_shown(&["You're all set"]);

    // Put off before anything is done, setup comes back another time.
    let mut driver =
        Driver::new(app_on(SetupPage::Welcome, Some(fresh_system()))).named("W2-later");
    driver.click("Set up later");
    assert!(driver.app.setup.is_none());
    assert_eq!(driver.app.setup_state, SetupState::Deferred);
    // The main window is back: its header chip is in reach again.
    driver.expect_shown(&["Checking remapping…"]);
}

/// The summary's row for a step, by the name it shows.
fn summary_row(step: Step) -> &'static str {
    match step {
        Step::Xremap => "xremap",
        Step::InputGroup => "Keyboard access",
        Step::Uinput => "Virtual keyboard",
        Step::Service => "Remapping",
    }
}

#[test]
fn finish_page_states_and_what_closing_records() {
    // F6: the checks are still running.
    let driver = Driver::new(app_on(SetupPage::Finish, None)).named("F6-checking");
    driver.expect_shown(&["Checking your system…", "Checking…", "Finish"]);

    // F1: everything in order.
    let mut driver = Driver::new(app_on(SetupPage::Finish, Some(ready()))).named("F1-all-set");
    driver.expect_shown(&[
        "You're all set",
        "xremap",
        "Installed",
        "Keyboard access",
        "Allowed",
        "Virtual keyboard",
        "Remapping",
        "Running",
        "Finish",
    ]);
    driver.expect_hidden("Continue setup");
    driver.expect_hidden("Set up later");
    driver.click("Finish");
    assert!(driver.app.setup.is_none());
    assert_eq!(driver.app.setup_state, SetupState::Complete);

    // F2: only a restart outstanding, which is complete all the same.
    let mut driver = Driver::new(app_on(
        SetupPage::Finish,
        Some(systems::waiting_for_login()),
    ))
    .named("F2-almost-there");
    driver.expect_shown(&[
        "Almost there",
        "Takes effect after a restart",
        "Starts after a restart",
        "Finish",
    ]);
    driver.click("Finish");
    assert_eq!(driver.app.setup_state, SetupState::Complete);

    // F3: a step left that Keyloom can act on resumes there.
    let mut driver = Driver::new(app_on(
        SetupPage::Finish,
        Some(systems::service_foreign_kept()),
    ))
    .named("F3-not-finished-resumable");
    driver.expect_shown(&[
        "Setup isn't finished",
        "Doesn't use Keyloom's remaps",
        "Set up later",
        "Continue setup",
    ]);
    driver.expect_hidden("Finish");
    driver.click("Continue setup");
    assert_eq!(driver.page(), SetupPage::Step(Step::Service));
    driver.expect_shown(&["Replace your xremap service?"]);
    driver.click("Back");
    assert_eq!(driver.page(), SetupPage::Step(Step::Uinput));
    driver.click("Continue");
    assert_eq!(
        driver.page(),
        SetupPage::Step(Step::Service),
        "the service still wants attention"
    );
    driver.click("Keep mine");
    assert_eq!(driver.page(), SetupPage::Finish);
    driver.click("Set up later");
    assert!(driver.app.setup.is_none());
    assert_eq!(driver.app.setup_state, SetupState::Deferred);

    // F4: steps left that nobody here can act on only close, unfinished.
    let mut driver = Driver::new(app_on(
        SetupPage::Finish,
        Some(facts(|f| {
            f.group = GroupCheck::NoGroup;
            f.unit = UnitCheck::Unavailable;
        })),
    ))
    .named("F4-not-finished-nothing-to-do");
    driver.expect_shown(&[
        "Setup isn't finished",
        "No input group",
        "No systemd user session",
        "Finish",
    ]);
    driver.expect_parts(&["select one to see why"]);
    driver.expect_hidden("Continue setup");
    driver.click("No systemd user session");
    assert_eq!(driver.page(), SetupPage::Step(Step::Service));
    driver.expect_shown(&["Remapping can't be turned on here"]);
    driver.click("Continue");
    driver.click("Finish");
    assert!(driver.app.setup.is_none());
    assert_eq!(driver.app.setup_state, SetupState::Deferred);
}

// ---- Transient states --------------------------------------------------

#[test]
fn a_failed_fix_is_explained_and_offered_again() {
    for (id, step, system, error, label) in [
        (
            "T2-cancelled",
            Step::InputGroup,
            systems::not_member(),
            ActionError::Cancelled,
            "Allow keyboard access",
        ),
        (
            "T3-not-authorized",
            Step::Uinput,
            systems::uinput_missing(),
            ActionError::NotAuthorized,
            "Allow virtual keyboard",
        ),
        (
            "T5a-download-failed",
            Step::Xremap,
            systems::xremap_missing(),
            ActionError::Failed("could not download xremap: connection timed out".to_owned()),
            "Install xremap",
        ),
        (
            "T5b-systemctl-failed",
            Step::Service,
            systems::service_missing(),
            ActionError::Failed(
                "Job for xremap.service failed because the control process exited with error code"
                    .to_owned(),
            ),
            "Turn on remapping",
        ),
    ] {
        if step == Step::Xremap && install::asset().is_none() {
            // Without a download for this processor there is no fix to fail.
            continue;
        }
        let explanation = format!("Couldn't finish this step: {error}");
        let mut driver = Driver::new(app_on_step(step, system, |s| {
            s.error = Some((step, error.clone()));
        }))
        .named(id);
        driver.expect_shown(&[&explanation, label]);
        driver.click(label);
        assert_eq!(
            driver.setup().busy,
            Some(step),
            "{id}: the fix is offered again"
        );
        driver.expect_hidden(&explanation);
    }
}

#[test]
fn copying_the_commands_is_acknowledged() {
    let mut driver = Driver::new(app_on_step(Step::InputGroup, systems::not_member(), |s| {
        s.details = true
    }))
    .named("T8-copied");
    driver.expect_shown(&["Copy"]);
    driver.click("Copy");
    assert!(driver.setup().copied);
    driver.expect_shown(&["Copied"]);
    driver.expect_hidden("Copy");

    // Once the acknowledgment has been shown long enough.
    let copy = driver.setup().copies;
    driver.deliver(Message::SetupCopyShown(copy));
    driver.expect_shown(&["Copy"]);
    driver.expect_hidden("Copied");
}

#[test]
fn the_service_file_can_be_saved_by_hand() {
    let mut driver = Driver::new(app_on_step(
        Step::Service,
        systems::service_missing(),
        |s| s.details = true,
    ))
    .named("T9-saving");
    driver.expect_shown(&["Save service file", "Check again", "Skip this step"]);
    driver.expect_hidden("Copy");

    driver.click("Save service file");
    assert!(driver.setup().saving);
    driver.expect_shown(&["Saving…"]);
    assert_eq!(driver.click("Turn on remapping"), 0, "one change at a time");

    // B5: the saved file turns the page into the one that shows how to
    // turn it on, with the command for it.
    driver.deliver(Message::SetupServiceSaved(Ok(())));
    assert!(driver.setup().probing);
    assert!(!driver.setup().saving);
    driver.deliver(Message::SetupProbed(facts(|f| {
        f.unit = UnitCheck::Keyloom {
            active: false,
            enabled: false,
        }
    })));
    driver.expect_shown(&["Turned off", "Turn on remapping", "Copy"]);
    driver.expect_parts(&["systemctl --user enable --now xremap.service"]);
    driver.expect_hidden("Save service file");
}

#[test]
fn details_stay_open_across_pages_and_close_with_the_page_that_failed() {
    let mut driver = Driver::new(app_on(SetupPage::Step(Step::Xremap), Some(fresh_system())))
        .named("T6-details");
    driver.click("Show details");
    driver.click("Set up later");
    assert!(driver.app.setup.is_none());

    // Reopened, the details start hidden again.
    driver.deliver(Message::MenuShowSetup);
    driver.deliver(Message::SetupProbed(fresh_system()));
    driver.click("Start setup");
    driver.expect_shown(&["Show details"]);
    driver.click("Show details");
    driver.click("Skip this step");
    assert_eq!(driver.page(), SetupPage::Step(Step::InputGroup));
    driver.expect_shown(&["Hide details"]);
    driver.expect_parts(&["sudo usermod -aG input blake"]);

    // T7: details a failure opened close again once the step moves on,
    // where the user's own choice would have stayed.
    driver.click("Hide details");
    driver.click("Allow keyboard access");
    driver.failed(Step::InputGroup, ActionError::NoPolkit, fresh_system());
    driver.expect_shown(&["Hide details"]);
    driver.click("Skip this step");
    assert_eq!(driver.page(), SetupPage::Step(Step::Uinput));
    driver.expect_shown(&["Show details"]);
}

// ---- Cross-cutting -----------------------------------------------------

#[test]
fn footer_buttons_stay_reachable_in_the_smallest_window() {
    // C3: a page taller than the window scrolls; the buttons do not.
    let mut driver = Driver::new(app_on_step(
        Step::Service,
        systems::service_foreign_kept(),
        |s| s.details = true,
    ))
    .at_minimum_size()
    .named("C3a-min-window-replace-details");
    driver.click("Replace service");
    assert_eq!(driver.setup().busy, Some(Step::Service));

    let mut driver = Driver::new(app_on(
        SetupPage::Finish,
        Some(systems::service_foreign_kept()),
    ))
    .at_minimum_size()
    .named("C3b-min-window-finish");
    driver.click("Continue setup");
    assert_eq!(driver.page(), SetupPage::Step(Step::Service));
    driver.click("Set up later");
    assert!(driver.app.setup.is_none());
}

#[test]
fn long_paths_are_shown_whole_in_the_details() {
    // C4
    let driver = Driver::new(app_on_step(Step::Service, systems::long_paths(), |s| {
        s.details = true;
    }))
    .named("C4-long-paths");
    driver.expect_parts(&[
        "add /home/Jo Doe/.config/xremap/keyloom.yml to its xremap command",
        "Your service: /home/Jo Doe/.config/systemd/user/xremap.service",
        "ExecStart=/opt/xremap/bin/xremap --watch=config --device 'Keychron K2'",
        "/home/Jo Doe/.config/xremap/work.yml",
    ]);
}

// ---- The main window ---------------------------------------------------

#[test]
fn setup_opens_from_the_menu_and_from_the_status_chip() {
    let mut driver = Driver::new(app()).named("menu");
    driver.expect_hidden("Set up remapping");
    driver.click("⋯");
    assert_eq!(driver.app.popover, Some(Popover::Menu));
    driver.click("Set up remapping");
    assert!(driver.app.popover.is_none());
    assert_eq!(driver.page(), SetupPage::Welcome);
    assert!(driver.setup().probing, "opening starts the checks");
    driver.expect_shown(&["Start setup"]);

    // With nothing set up, the header's status chip leads to setup too.
    let mut driver = Driver::new(app()).named("chip");
    driver.deliver(Message::ServiceStatus(service::Status::NotFound));
    driver.expect_shown(&["Remapping not set up"]);
    driver.click("Remapping not set up");
    assert_eq!(driver.page(), SetupPage::Welcome);
}

// ---- Storyboards -------------------------------------------------------

/// The flows of the matrix, driven by clicks. Each takes the frames it
/// would picture, so the screenshot tests can record the same flow.
pub mod storyboards {
    use super::*;

    /// SB1: everything setup has to do on a system with nothing.
    pub fn fresh_system(recorder: Recorder) {
        let mut driver = Driver::with_recorder(app(), recorder);
        driver.click("⋯");
        driver.click("Set up remapping");
        assert_eq!(driver.page(), SetupPage::Welcome);
        driver.frame("welcome-checking");

        driver.deliver(Message::SetupProbed(super::fresh_system()));
        driver.frame("welcome");
        driver.click("Start setup");
        assert_eq!(driver.page(), SetupPage::Step(Step::Xremap));
        driver.expect_shown(&["Not installed", "Install xremap"]);
        driver.frame("xremap-not-installed");
        driver.click("Install xremap");
        driver.expect_shown(&["Installing…"]);
        driver.frame("xremap-installing");
        let mut system = super::fresh_system();
        system.xremap = found(true, Some(install::RELEASE), Some(vec![Desktop::Cosmic]));
        driver.fixed(Step::Xremap, system.clone());

        assert_eq!(driver.page(), SetupPage::Step(Step::InputGroup));
        driver.expect_shown(&["Not allowed yet", "Allow keyboard access"]);
        driver.frame("group-not-allowed");
        driver.click("Allow keyboard access");
        driver.expect_shown(&[APPROVAL]);
        driver.frame("group-approval");
        system.group = GroupCheck::NeedsLogin;
        driver.fixed(Step::InputGroup, system.clone());

        assert_eq!(driver.page(), SetupPage::Step(Step::Uinput));
        driver.expect_shown(&["Not set up", "Allow the virtual keyboard"]);
        driver.frame("uinput-not-set-up");
        driver.click("Allow virtual keyboard");
        driver.expect_shown(&[APPROVAL]);
        driver.frame("uinput-approval");
        system.uinput = UinputCheck::NotWritable {
            rule_installed: true,
        };
        driver.fixed(Step::Uinput, system.clone());

        assert_eq!(driver.page(), SetupPage::Step(Step::Service));
        driver.expect_shown(&["Not set up", "Turn on remapping"]);
        driver.frame("service-not-set-up");
        driver.click("Turn on remapping");
        driver.expect_shown(&["Turning on…"]);
        driver.frame("service-turning-on");
        system.unit = UnitCheck::Keyloom {
            active: false,
            enabled: true,
        };
        driver.fixed(Step::Service, system);

        assert_eq!(driver.page(), SetupPage::Finish);
        driver.expect_shown(&["Almost there", "Finish"]);
        driver.frame("finish-almost-there");
        driver.click("Finish");
        assert!(driver.app.setup.is_none());
        assert_eq!(driver.app.setup_state, SetupState::Complete);
        driver.deliver(Message::ServiceStatus(service::Status::Inactive));
        driver.expect_shown(&["Remapping Paused"]);
        driver.frame("main-window");
    }

    /// SB2: a service of the user's own, kept.
    pub fn foreign_unit_kept(recorder: Recorder) {
        let mut driver = Driver::with_recorder(
            app_on(SetupPage::Welcome, Some(systems::service_foreign_kept())),
            recorder,
        );
        driver.frame("welcome");

        // The earlier steps are in order, so setup goes straight to the service.
        driver.click("Start setup");
        assert_eq!(driver.page(), SetupPage::Step(Step::Service));
        driver.expect_shown(&[
            "Doesn't use Keyloom's remaps",
            "Replace your xremap service?",
            "Keep mine",
            "Replace service",
        ]);
        driver.frame("service-replace");

        driver.click("Keep mine");
        assert_eq!(driver.page(), SetupPage::Finish);
        driver.expect_shown(&["Setup isn't finished", "Continue setup"]);
        driver.frame("finish-not-finished");

        driver.click("Set up later");
        assert!(driver.app.setup.is_none());
        assert_eq!(driver.app.setup_state, SetupState::Deferred);
        // Their unit is running, which is all the header's chip knows.
        driver.deliver(Message::ServiceStatus(service::Status::Active));
        driver.expect_shown(&["Remapping Enabled"]);
        driver.frame("main-window-chip");
    }

    /// SB3: the password prompt dismissed, then answered.
    pub fn authorization_refused_then_granted(recorder: Recorder) {
        let mut driver = Driver::with_recorder(
            app_on(
                SetupPage::Step(Step::InputGroup),
                Some(systems::not_member()),
            ),
            recorder,
        );
        driver.frame("group-not-allowed");

        driver.click("Allow keyboard access");
        driver.expect_shown(&[APPROVAL]);
        driver.frame("approval");
        driver.failed(
            Step::InputGroup,
            ActionError::Cancelled,
            systems::not_member(),
        );
        driver.expect_shown(&[
            "Couldn't finish this step: authorization was cancelled, so nothing was changed",
            "Allow keyboard access",
        ]);
        assert!(
            !driver.setup().details,
            "a dismissed prompt opens no details"
        );
        driver.frame("cancelled");

        driver.click("Allow keyboard access");
        driver.expect_shown(&[APPROVAL]);
        driver.frame("approval-again");
        driver.fixed(
            Step::InputGroup,
            facts(|f| f.group = GroupCheck::NeedsLogin),
        );
        assert_eq!(driver.page(), SetupPage::Finish);
        driver.expect_shown(&["Almost there"]);
        driver.frame("finish-almost-there");
        driver.click(summary_row(Step::InputGroup));
        assert_eq!(driver.page(), SetupPage::Step(Step::InputGroup));
        driver.expect_shown(&[
            "Takes effect after a restart",
            "Keyboard access is set up",
            "Continue",
        ]);
        driver.frame("group-next-login");
    }

    /// SB4: without pkexec, the step is done by hand.
    pub fn no_polkit(recorder: Recorder) {
        let mut driver = Driver::with_recorder(
            app_on(
                SetupPage::Step(Step::InputGroup),
                Some(systems::not_member()),
            ),
            recorder,
        );
        driver.frame("group-not-allowed");

        driver.click("Allow keyboard access");
        driver.failed(
            Step::InputGroup,
            ActionError::NoPolkit,
            systems::not_member(),
        );
        // T4: the fix becomes a recheck, and the details open on their
        // own with the command to run.
        driver.expect_shown(&["Check again", "Hide details"]);
        driver.expect_parts(&["pkexec is not installed", "sudo usermod -aG input blake"]);
        driver.frame("no-polkit-details");

        // The user ran the command by hand and checks again.
        driver.click("Check again");
        assert!(driver.setup().probing);
        driver.expect_shown(&["Checking…"]);
        driver.frame("checking");
        driver.deliver(Message::SetupProbed(facts(|f| {
            f.group = GroupCheck::NeedsLogin
        })));
        driver.expect_shown(&["Takes effect after a restart", "Keyboard access is set up"]);
        driver.frame("group-next-login");
    }

    /// SB5 (F5): reopened after completion, every step continues.
    pub fn reopened_after_completion(recorder: Recorder) {
        let mut app = app_on(SetupPage::Welcome, Some(ready()));
        app.setup_state = SetupState::Complete;
        let mut driver = Driver::with_recorder(app, recorder);
        driver.frame("welcome");

        driver.click("Start setup");
        assert_eq!(driver.page(), SetupPage::Finish);
        for step in Step::ALL {
            driver.click(summary_row(step));
            assert_eq!(driver.page(), SetupPage::Step(step));
            driver.expect_shown(&["Continue"]);
            driver.frame(&format!("{step:?}").to_lowercase());
            driver.click("Continue");
            assert_eq!(driver.page(), SetupPage::Finish);
        }
        driver.expect_shown(&["You're all set"]);
        driver.frame("finish-all-set");
        driver.click("Finish");
        assert!(driver.app.setup.is_none());
        assert_eq!(driver.app.setup_state, SetupState::Complete);
        driver.deliver(Message::ServiceStatus(service::Status::Active));
        driver.frame("main-window");
    }

    /// SB6: Keyloom's own download of xremap is out of date.
    pub fn download_out_of_date(recorder: Recorder) {
        let mut driver = Driver::with_recorder(
            app_on(SetupPage::Welcome, Some(systems::xremap_outdated())),
            recorder,
        );
        driver.frame("welcome");
        driver.click("Start setup");
        driver.expect_shown(&["Update available", "Update xremap"]);
        driver.frame("update-available");
        driver.click("Update xremap");
        driver.expect_shown(&["Updating…"]);
        driver.frame("updating");
        driver.fixed(
            Step::Xremap,
            facts(|f| f.xremap = found(true, Some(install::RELEASE), Some(vec![Desktop::Cosmic]))),
        );
        assert_eq!(driver.page(), SetupPage::Finish);
        driver.expect_shown(&["You're all set"]);
        driver.frame("finish-all-set");
    }
}

#[test]
fn storyboard_sb1_fresh_system() {
    if install::asset().is_none() {
        // The flow installs xremap, which this build cannot offer.
        return;
    }
    storyboards::fresh_system(None);
}

#[test]
fn storyboard_sb2_foreign_unit_kept() {
    storyboards::foreign_unit_kept(None);
}

#[test]
fn storyboard_sb3_authorization_refused_then_granted() {
    storyboards::authorization_refused_then_granted(None);
}

#[test]
fn storyboard_sb4_no_polkit() {
    storyboards::no_polkit(None);
}

#[test]
fn storyboard_sb5_reopened_after_completion() {
    storyboards::reopened_after_completion(None);
}

#[test]
fn storyboard_sb6_download_out_of_date() {
    if install::asset().is_none() {
        return;
    }
    storyboards::download_out_of_date(None);
}

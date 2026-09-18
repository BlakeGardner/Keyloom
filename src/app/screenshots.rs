//! Screenshots of the window for review, rendered without a display.
//!
//! These tests never assert on pixels: each stages a state, renders
//! the window the way the runtime composes it, and writes a PNG under
//! `target/setup-shots/` for a person (or CI's artifact upload) to look
//! at. They are ignored by default so the ordinary test run stays
//! fast; run them with
//! `cargo test --locked screenshots -- --ignored`
//! (or one section, e.g. `screenshots::service`).
//!
//! A state is staged from a [`setup::Facts`] literal and the wizard's
//! own fields, never from the machine the tests run on, so every test
//! starts clean and nothing on the system is read or changed. The
//! renderer is the software one (tiny-skia), so the same state gives
//! the same image on a workstation with a GPU and on a CI runner.
//!
//! The states, and the identifiers the files are named after, are
//! listed in `docs/Setup_Test_Matrix.md`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cosmic::Application;
use cosmic::iced::core::renderer::{Headless, Style};
use cosmic::iced::core::theme::Base;
use cosmic::iced::core::{Event, Pixels, Size, clipboard, mouse, time::Instant, window};
use cosmic::iced::runtime::UserInterface;
use cosmic::iced::runtime::user_interface::Cache;
use cosmic::widget;

use super::*;
use crate::install;
use crate::service;
use crate::session::{Desktop, Session};
use crate::setup::{ActionError, Facts, GroupCheck, Step, UinputCheck, UnitCheck, XremapCheck};

/// A window size to render at: in logical pixels, and in the pixels of
/// the image, which is twice the logical size (like a HiDPI display)
/// so small text stays readable when zoomed in.
struct Viewport {
    logical: Size,
    pixels: Size<u32>,
}

const SCALE: f32 = 2.0;

/// The window's default size (`src/main.rs`), so pages are captured
/// with exactly the room they get, clipping included.
const WINDOW: Viewport = Viewport {
    logical: Size::new(1210.0, 620.0),
    pixels: Size::new(2420, 1240),
};

/// The smallest the window can be made (`src/main.rs`).
const MIN_WINDOW: Viewport = Viewport {
    logical: Size::new(760.0, 480.0),
    pixels: Size::new(1520, 960),
};

/// The window as the runtime composes it: the header bar with the
/// application's own header widgets, the content beneath, and any
/// dialog centered over both (`libcosmic/src/app/mod.rs`).
fn window(app: &App) -> Element<'_, Message> {
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

/// Where a named screenshot goes: `target/setup-shots/<name>.png`.
fn shot(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("setup-shots")
        .join(format!("{name}.png"))
}

/// A renderer kept for the captures of one test: creating one loads
/// the system's fonts, which is the slow part.
struct Shots {
    renderer: cosmic::Renderer,
}

impl Shots {
    /// The software renderer, with the running application's font.
    fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime to create the renderer on");
        let renderer = runtime
            .block_on(<cosmic::Renderer as Headless>::new(
                cosmic::font::default(),
                Pixels(14.0),
                Some("tiny-skia"),
            ))
            .expect("the tiny-skia renderer needs no GPU");
        Self { renderer }
    }

    /// Render the window in the dark theme at its default size and
    /// write it out.
    fn capture(&mut self, app: &App, name: &str) {
        self.capture_as(app, name, &cosmic::Theme::dark(), &WINDOW);
    }

    /// Lay the window out, draw it once, and write the pixels as a PNG.
    fn capture_as(&mut self, app: &App, name: &str, theme: &cosmic::Theme, viewport: &Viewport) {
        let mut interface = UserInterface::build(
            window(app),
            viewport.logical,
            Cache::default(),
            &mut self.renderer,
        );
        let mut messages = Vec::new();
        let _ = interface.update(
            &[Event::Window(
                window::Event::RedrawRequested(Instant::now()),
            )],
            mouse::Cursor::Unavailable,
            &mut self.renderer,
            &mut clipboard::Null,
            &mut messages,
        );
        let base = theme.base();
        interface.draw(
            &mut self.renderer,
            theme,
            &Style {
                icon_color: base.text_color,
                text_color: base.text_color,
                scale_factor: f64::from(SCALE),
            },
            mouse::Cursor::Unavailable,
        );
        // The interface must outlive the screenshot: the renderer keeps
        // only weak references to the text it was asked to draw, and
        // the paragraphs themselves live in the widget tree.
        let rgba = self
            .renderer
            .screenshot(viewport.pixels, SCALE, base.background_color);
        drop(interface);

        let path = shot(name);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).expect("a writable target directory");
        }
        let file = fs::File::create(&path).expect("the screenshot file");
        let mut encoder = png::Encoder::new(
            io::BufWriter::new(file),
            viewport.pixels.width,
            viewport.pixels.height,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("the PNG header");
        writer.write_image_data(&rgba).expect("the PNG pixels");
        writer.finish().expect("the PNG to finish");
        eprintln!("screenshot: {}", path.display());
    }
}

// ---- Staging -----------------------------------------------------------

const CONFIG: &str = "/home/blake/.config/xremap/keyloom.yml";
const UNIT_PATH: &str = "/home/blake/.config/systemd/user/xremap.service";

/// A system where everything is in order: the user's own xremap on a
/// COSMIC Wayland session, access granted, Keyloom's unit running.
fn ready() -> Facts {
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
fn facts(edit: impl FnOnce(&mut Facts)) -> Facts {
    let mut facts = ready();
    edit(&mut facts);
    facts
}

fn found(managed: bool, version: Option<&str>, desktops: Option<Vec<Desktop>>) -> XremapCheck {
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
fn foreign(reads_config: bool, active: bool) -> UnitCheck {
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

/// The wizard open on a page with the checks done (or, without facts,
/// still running).
fn app_on(page: SetupPage, facts: Option<Facts>) -> App {
    let mut app = App::init(Core::default(), ()).0;
    app.setup = Some(Setup {
        page,
        probing: facts.is_none(),
        facts,
        ..Setup::new()
    });
    app
}

/// The wizard open on a step, with the open wizard's fields adjusted.
fn app_on_step(step: Step, facts: Facts, edit: impl FnOnce(&mut Setup)) -> App {
    let mut app = app_on(SetupPage::Step(step), Some(facts));
    edit(setup_of(&mut app));
    app
}

fn setup_of(app: &mut App) -> &mut Setup {
    app.setup.as_mut().expect("setup is open")
}

/// Feed the wizard a message, as the runtime would.
fn act(app: &mut App, message: Message) {
    let _ = app.update(message);
}

/// Render every row of a step's table.
fn capture_rows(shots: &mut Shots, dir: &str, step: Step, rows: Vec<(&str, Facts)>) {
    for (id, facts) in rows {
        let app = app_on(SetupPage::Step(step), Some(facts));
        shots.capture(&app, &format!("{dir}/{id}"));
    }
}

// ---- Rows of the matrix ------------------------------------------------

/// A state per row, for the steps whose rows other sections reuse.
mod rows {
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
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn pages_around_the_steps() {
    let mut shots = Shots::new();

    shots.capture(&app_on(SetupPage::Welcome, None), "welcome/W1-checking");
    shots.capture(&app_on(SetupPage::Welcome, Some(ready())), "welcome/W2");

    let finish = [
        ("F1-all-set", Some(ready())),
        ("F2-almost-there", Some(rows::waiting_for_login())),
        (
            "F3-not-finished-resumable",
            Some(rows::service_foreign_kept()),
        ),
        (
            "F4-not-finished-nothing-to-do",
            Some(facts(|f| {
                f.group = GroupCheck::NoGroup;
                f.unit = UnitCheck::Unavailable;
            })),
        ),
        ("F6-checking", None),
    ];
    for (id, facts) in finish {
        shots.capture(&app_on(SetupPage::Finish, facts), &format!("finish/{id}"));
    }
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn xremap() {
    let gnome = |f: &mut Facts| {
        f.xremap = found(false, Some(install::RELEASE), Some(vec![Desktop::Gnome]));
        f.session.desktop = Some(Desktop::Gnome);
    };
    capture_rows(
        &mut Shots::new(),
        "xremap",
        Step::Xremap,
        vec![
            ("X1-installed", ready()),
            (
                "X2-installed-keyloom-download",
                facts(|f| {
                    f.xremap = found(true, Some(install::RELEASE), Some(vec![Desktop::Cosmic]))
                }),
            ),
            ("X3-update-available", rows::xremap_outdated()),
            (
                "X4-version-unknown",
                facts(|f| f.xremap = found(false, None, Some(vec![Desktop::Cosmic]))),
            ),
            ("X5-not-installed", rows::xremap_missing()),
            // X6 (no release for this processor) depends on the build's
            // architecture, not on the facts, so it cannot be staged here.
            ("XA2-gnome-wayland", facts(gnome)),
            (
                "XA3-gnome-x11",
                facts(|f| {
                    gnome(f);
                    f.session.x11 = true;
                }),
            ),
            (
                "XA4-unsupported-desktop",
                facts(|f| {
                    f.xremap = found(
                        false,
                        Some(install::RELEASE),
                        Some(vec![Desktop::Gnome, Desktop::Kde]),
                    );
                }),
            ),
            (
                "XA5-unreported",
                facts(|f| f.xremap = found(false, Some("0.10.0"), None)),
            ),
            ("XA6-unknown-desktop", facts(|f| f.session.desktop = None)),
        ],
    );
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn keyboard_access() {
    capture_rows(
        &mut Shots::new(),
        "group",
        Step::InputGroup,
        vec![
            ("G1-allowed", ready()),
            ("G2-next-login", facts(|f| f.group = GroupCheck::NeedsLogin)),
            ("G3-not-allowed", rows::not_member()),
            (
                "G4-not-allowed-user-unknown",
                facts(|f| {
                    f.group = GroupCheck::NotMember;
                    f.user = None;
                }),
            ),
            ("G5-no-group", facts(|f| f.group = GroupCheck::NoGroup)),
        ],
    );
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn virtual_keyboard() {
    let not_writable = |rule_installed: bool| UinputCheck::NotWritable { rule_installed };
    capture_rows(
        &mut Shots::new(),
        "uinput",
        Step::Uinput,
        vec![
            ("U1-allowed", ready()),
            ("U2-missing", rows::uinput_missing()),
            (
                "U3-missing-rule-installed",
                facts(|f| {
                    f.uinput = UinputCheck::Missing {
                        rule_installed: true,
                    }
                }),
            ),
            ("U4-not-allowed", facts(|f| f.uinput = not_writable(false))),
            (
                "U5-next-login",
                facts(|f| {
                    f.uinput = not_writable(true);
                    f.group = GroupCheck::NeedsLogin;
                }),
            ),
            ("U6-not-in-effect", facts(|f| f.uinput = not_writable(true))),
        ],
    );
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn service() {
    let keyloom = |active: bool, enabled: bool| UnitCheck::Keyloom { active, enabled };
    capture_rows(
        &mut Shots::new(),
        "service",
        Step::Service,
        vec![
            ("S1-running", ready()),
            ("S2-set-up-starts-at-login", rows::waiting_for_login()),
            (
                "S3-set-up-waiting-for-access",
                facts(|f| {
                    f.uinput = UinputCheck::NotWritable {
                        rule_installed: false,
                    };
                    f.unit = keyloom(false, true);
                }),
            ),
            ("S4-not-running", facts(|f| f.unit = keyloom(false, true))),
            ("S5-not-at-login", facts(|f| f.unit = keyloom(true, false))),
            ("S6-turned-off", facts(|f| f.unit = keyloom(false, false))),
            ("S7-stale-running", rows::service_stale()),
            (
                "S8-stale-stopped",
                facts(|f| f.unit = UnitCheck::Stale { active: false }),
            ),
            ("S9-foreign-works", facts(|f| f.unit = foreign(true, true))),
            ("S10-foreign-stopped", rows::service_foreign_stopped()),
            ("S11-foreign-replace", rows::service_foreign_kept()),
            (
                "S12-foreign-replace-stopped",
                facts(|f| f.unit = foreign(false, false)),
            ),
            (
                "S13-foreign-unreadable",
                facts(|f| {
                    f.unit = UnitCheck::Foreign {
                        exec_start: String::new(),
                        reads_config: false,
                        active: true,
                        path: Some(PathBuf::from(UNIT_PATH)),
                    }
                }),
            ),
            ("S14-not-set-up", rows::service_missing()),
            (
                "S15-needs-xremap",
                facts(|f| {
                    f.xremap = XremapCheck::Missing;
                    f.unit = UnitCheck::Missing;
                }),
            ),
            (
                "S16-no-home",
                facts(|f| {
                    f.config = None;
                    f.unit = UnitCheck::Missing;
                }),
            ),
            ("S17-no-systemd", facts(|f| f.unit = UnitCheck::Unavailable)),
            (
                "S18-no-systemd-nothing-known",
                facts(|f| {
                    f.xremap = XremapCheck::Missing;
                    f.config = None;
                    f.unit = UnitCheck::Unavailable;
                }),
            ),
        ],
    );
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn transient_states() {
    let mut shots = Shots::new();
    let busy = |step: Step| move |s: &mut Setup| s.busy = Some(step);
    let failed =
        |step: Step, error: ActionError| move |s: &mut Setup| s.error = Some((step, error));
    let details = |s: &mut Setup| s.details = true;

    let states: Vec<(&str, App)> = vec![
        ("T1a-installing", app_on_step(Step::Xremap, rows::xremap_missing(), busy(Step::Xremap))),
        ("T1b-updating", app_on_step(Step::Xremap, rows::xremap_outdated(), busy(Step::Xremap))),
        ("T1c-group-approval", app_on_step(Step::InputGroup, rows::not_member(), busy(Step::InputGroup))),
        ("T1d-uinput-approval", app_on_step(Step::Uinput, rows::uinput_missing(), busy(Step::Uinput))),
        ("T1e-turning-on", app_on_step(Step::Service, rows::service_missing(), busy(Step::Service))),
        ("T1f-replacing", app_on_step(Step::Service, rows::service_foreign_kept(), busy(Step::Service))),
        ("T1g-updating-service", app_on_step(Step::Service, rows::service_stale(), busy(Step::Service))),
        ("T1h-starting", app_on_step(Step::Service, rows::service_foreign_stopped(), busy(Step::Service))),
        (
            "T2-cancelled",
            app_on_step(Step::InputGroup, rows::not_member(), failed(Step::InputGroup, ActionError::Cancelled)),
        ),
        (
            "T3-not-authorized",
            app_on_step(Step::Uinput, rows::uinput_missing(), failed(Step::Uinput, ActionError::NotAuthorized)),
        ),
        (
            "T4-no-polkit",
            app_on_step(Step::InputGroup, rows::not_member(), |s| {
                s.error = Some((Step::InputGroup, ActionError::NoPolkit));
                s.details = true;
                s.details_for_failure = true;
            }),
        ),
        (
            "T5a-download-failed",
            app_on_step(
                Step::Xremap,
                rows::xremap_missing(),
                failed(
                    Step::Xremap,
                    ActionError::Failed("could not download xremap: connection timed out".to_owned()),
                ),
            ),
        ),
        (
            "T5b-systemctl-failed",
            app_on_step(
                Step::Service,
                rows::service_missing(),
                failed(
                    Step::Service,
                    ActionError::Failed(
                        "Job for xremap.service failed because the control process exited with error code"
                            .to_owned(),
                    ),
                ),
            ),
        ),
        ("T6a-details-save-file", app_on_step(Step::Service, rows::service_missing(), details)),
        ("T6b-details-commands", app_on_step(Step::InputGroup, rows::not_member(), details)),
        ("T6c-details-installed", app_on_step(Step::Xremap, ready(), details)),
        (
            "T8-copied",
            app_on_step(Step::InputGroup, rows::not_member(), |s| {
                s.details = true;
                s.copied = true;
            }),
        ),
        (
            "T9-saving",
            app_on_step(Step::Service, rows::service_missing(), |s| {
                s.details = true;
                s.saving = true;
            }),
        ),
    ];
    for (id, app) in &states {
        shots.capture(app, &format!("transient/{id}"));
    }
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn cross_cutting_variations() {
    let mut shots = Shots::new();
    let light = cosmic::Theme::light();

    let welcome = app_on(SetupPage::Welcome, Some(ready()));
    shots.capture_as(&welcome, "variations/C2a-light-welcome", &light, &WINDOW);
    let replace = app_on(
        SetupPage::Step(Step::Service),
        Some(rows::service_foreign_kept()),
    );
    shots.capture_as(&replace, "variations/C2b-light-replace", &light, &WINDOW);

    let replace_details = app_on_step(Step::Service, rows::service_foreign_kept(), |s| {
        s.details = true
    });
    shots.capture_as(
        &replace_details,
        "variations/C3a-min-window-replace-details",
        &cosmic::Theme::dark(),
        &MIN_WINDOW,
    );
    let finish = app_on(SetupPage::Finish, Some(rows::service_foreign_kept()));
    shots.capture_as(
        &finish,
        "variations/C3b-min-window-finish",
        &cosmic::Theme::dark(),
        &MIN_WINDOW,
    );

    let long_paths = app_on_step(
        Step::Service,
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
        }),
        |s| s.details = true,
    );
    shots.capture(&long_paths, "variations/C4-long-paths");
}

// ---- Storyboards -------------------------------------------------------

/// Frames of one flow, numbered in order under `storyboard/<id>/`.
struct Storyboard {
    shots: Shots,
    id: &'static str,
    frame: u32,
}

impl Storyboard {
    fn new(id: &'static str) -> Self {
        Self {
            shots: Shots::new(),
            id,
            frame: 0,
        }
    }

    fn frame(&mut self, app: &App, label: &str) {
        self.frame += 1;
        let name = format!("storyboard/{}/{:02}-{label}", self.id, self.frame);
        self.shots.capture(app, &name);
    }
}

/// Everything setup has to do on a system with nothing: xremap missing,
/// no access, no service.
fn fresh_system() -> Facts {
    facts(|f| {
        f.xremap = XremapCheck::Missing;
        f.group = GroupCheck::NotMember;
        f.uinput = UinputCheck::Missing {
            rule_installed: false,
        };
        f.unit = UnitCheck::Missing;
    })
}

/// A fix that worked, then the checks agreeing: the page moves on.
fn fixed(app: &mut App, step: Step, after: Facts) {
    act(
        app,
        Message::SetupActed {
            step,
            result: Ok(()),
        },
    );
    act(app, Message::SetupProbed(after));
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb1_fresh_system() {
    let mut board = Storyboard::new("SB1-fresh-system");
    let mut app = App::init(Core::default(), ()).0;
    app.setup = Some(Setup::new());
    board.frame(&app, "welcome-checking");

    act(&mut app, Message::SetupProbed(fresh_system()));
    board.frame(&app, "welcome");
    act(&mut app, Message::SetupContinue);
    board.frame(&app, "xremap-not-installed");
    act(&mut app, Message::SetupAct(Step::Xremap));
    board.frame(&app, "xremap-installing");
    let mut system = fresh_system();
    system.xremap = found(true, Some(install::RELEASE), Some(vec![Desktop::Cosmic]));
    fixed(&mut app, Step::Xremap, system.clone());
    board.frame(&app, "group-not-allowed");

    act(&mut app, Message::SetupAct(Step::InputGroup));
    board.frame(&app, "group-approval");
    system.group = GroupCheck::NeedsLogin;
    fixed(&mut app, Step::InputGroup, system.clone());
    board.frame(&app, "uinput-not-set-up");

    act(&mut app, Message::SetupAct(Step::Uinput));
    board.frame(&app, "uinput-approval");
    system.uinput = UinputCheck::NotWritable {
        rule_installed: true,
    };
    fixed(&mut app, Step::Uinput, system.clone());
    board.frame(&app, "service-not-set-up");

    act(&mut app, Message::SetupAct(Step::Service));
    board.frame(&app, "service-turning-on");
    system.unit = UnitCheck::Keyloom {
        active: false,
        enabled: true,
    };
    fixed(&mut app, Step::Service, system);
    board.frame(&app, "finish-almost-there");
    assert_eq!(setup_of(&mut app).page, SetupPage::Finish);

    act(&mut app, Message::SetupFinish);
    app.service = Some(service::Status::Inactive);
    board.frame(&app, "main-window");
    assert_eq!(app.setup_state, SetupState::Complete);
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb2_foreign_unit_kept() {
    let mut board = Storyboard::new("SB2-foreign-unit-kept");
    let mut app = app_on(SetupPage::Welcome, Some(rows::service_foreign_kept()));
    board.frame(&app, "welcome");

    // The earlier steps are in order, so setup goes straight to the service.
    act(&mut app, Message::SetupContinue);
    assert_eq!(setup_of(&mut app).page, SetupPage::Step(Step::Service));
    board.frame(&app, "service-replace");

    // "Keep mine" only continues.
    act(&mut app, Message::SetupContinue);
    board.frame(&app, "finish-not-finished");

    act(&mut app, Message::SetupLater);
    assert_eq!(app.setup_state, SetupState::Deferred);
    // Their unit is running, which is all the header's chip knows.
    app.service = Some(service::Status::Active);
    board.frame(&app, "main-window-chip");
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb3_authorization_refused_then_granted() {
    let mut board = Storyboard::new("SB3-authorization-refused");
    let mut app = app_on(SetupPage::Step(Step::InputGroup), Some(rows::not_member()));
    board.frame(&app, "group-not-allowed");

    act(&mut app, Message::SetupAct(Step::InputGroup));
    board.frame(&app, "approval");
    act(
        &mut app,
        Message::SetupActed {
            step: Step::InputGroup,
            result: Err(ActionError::Cancelled),
        },
    );
    act(&mut app, Message::SetupProbed(rows::not_member()));
    board.frame(&app, "cancelled");

    act(&mut app, Message::SetupAct(Step::InputGroup));
    board.frame(&app, "approval-again");
    fixed(
        &mut app,
        Step::InputGroup,
        facts(|f| f.group = GroupCheck::NeedsLogin),
    );
    board.frame(&app, "finish-almost-there");
    act(
        &mut app,
        Message::SetupPage(SetupPage::Step(Step::InputGroup)),
    );
    board.frame(&app, "group-next-login");
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb4_no_polkit() {
    let mut board = Storyboard::new("SB4-no-polkit");
    let mut app = app_on(SetupPage::Step(Step::InputGroup), Some(rows::not_member()));
    board.frame(&app, "group-not-allowed");

    act(&mut app, Message::SetupAct(Step::InputGroup));
    act(
        &mut app,
        Message::SetupActed {
            step: Step::InputGroup,
            result: Err(ActionError::NoPolkit),
        },
    );
    act(&mut app, Message::SetupProbed(rows::not_member()));
    board.frame(&app, "no-polkit-details");

    // The user ran the command by hand and checks again.
    act(&mut app, Message::SetupRecheck);
    board.frame(&app, "checking");
    act(
        &mut app,
        Message::SetupProbed(facts(|f| f.group = GroupCheck::NeedsLogin)),
    );
    board.frame(&app, "group-next-login");
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb5_reopened_after_completion() {
    let mut board = Storyboard::new("SB5-reopened");
    let mut app = app_on(SetupPage::Welcome, Some(ready()));
    app.setup_state = SetupState::Complete;
    board.frame(&app, "welcome");
    for step in Step::ALL {
        act(&mut app, Message::SetupPage(SetupPage::Step(step)));
        board.frame(&app, &format!("{step:?}").to_lowercase());
    }
    act(&mut app, Message::SetupPage(SetupPage::Finish));
    board.frame(&app, "finish-all-set");
    act(&mut app, Message::SetupFinish);
    app.service = Some(service::Status::Active);
    board.frame(&app, "main-window");
    assert_eq!(app.setup_state, SetupState::Complete);
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb6_download_out_of_date() {
    let mut board = Storyboard::new("SB6-download-out-of-date");
    let mut app = app_on(SetupPage::Welcome, Some(rows::xremap_outdated()));
    board.frame(&app, "welcome");
    act(&mut app, Message::SetupContinue);
    board.frame(&app, "update-available");
    act(&mut app, Message::SetupAct(Step::Xremap));
    board.frame(&app, "updating");
    fixed(
        &mut app,
        Step::Xremap,
        facts(|f| f.xremap = found(true, Some(install::RELEASE), Some(vec![Desktop::Cosmic]))),
    );
    board.frame(&app, "finish-all-set");
    assert_eq!(setup_of(&mut app).page, SetupPage::Finish);
}

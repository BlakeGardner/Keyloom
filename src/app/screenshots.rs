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
//! The states pictured are the rows the interface tests assert on
//! (`e2e`), and the storyboards are those tests' flows with a frame
//! captured at each point they name: an image shows what a test has
//! already checked, for the eye to judge what an assertion cannot,
//! such as wrapping, contrast, and the look of a page. A state is staged
//! from a [`setup::Facts`] literal (`staging`), never from the machine
//! the tests run on, and the renderer is the software one (tiny-skia),
//! so the same state gives the same image on a workstation with a GPU
//! and on a CI runner.
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

use super::e2e::{self, Recorder, Row};
use super::staging::{self, app_on, app_on_step, facts, ready, systems, window};
use super::*;
use crate::setup::{ActionError, GroupCheck, Step, UnitCheck};

/// A window size to render at: in logical pixels, and in the pixels of
/// the image, which is twice the logical size (like a HiDPI display)
/// so small text stays readable when zoomed in.
struct Viewport {
    logical: Size,
    pixels: Size<u32>,
}

const SCALE: f32 = 2.0;

/// The window's default size, so pages are captured with exactly the
/// room they get, clipping included.
const WINDOW: Viewport = Viewport {
    logical: staging::WINDOW,
    pixels: Size::new(2420, 1240),
};

/// The smallest the window can be made.
const MIN_WINDOW: Viewport = Viewport {
    logical: staging::MIN_WINDOW,
    pixels: Size::new(1520, 960),
};

/// Where a named screenshot goes: `target/setup-shots/<name>.png`.
fn shot(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("setup-shots")
        .join(format!("{name}.png"))
}

/// A renderer kept for the captures of one test: creating one loads
/// the system's fonts, which is the slow part.
pub(super) struct Shots {
    renderer: cosmic::Renderer,
}

impl Shots {
    /// The software renderer, with the running application's font.
    pub(super) fn new() -> Self {
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

    /// Render the window in the dark theme at any logical size, as the
    /// interface tests picture a failure at the size they were driving.
    pub(super) fn capture_at(&mut self, app: &App, name: &str, logical: Size) {
        // Twice a window's whole pixels is well within `u32`.
        let pixels = |length: f32| (length * SCALE).round() as u32;
        let viewport = Viewport {
            logical,
            pixels: Size::new(pixels(logical.width), pixels(logical.height)),
        };
        self.capture_as(app, name, &cosmic::Theme::dark(), &viewport);
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

/// Render every row of a step's table, as the interface tests stage it.
fn capture_rows(shots: &mut Shots, dir: &str, step: Step, rows: Vec<Row>) {
    for row in rows {
        let app = app_on(SetupPage::Step(step), Some(row.facts));
        shots.capture(&app, &format!("{dir}/{}", row.id));
    }
}

// ---- Rows of the matrix ------------------------------------------------

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn pages_around_the_steps() {
    let mut shots = Shots::new();

    shots.capture(&app_on(SetupPage::Welcome, None), "welcome/W1-checking");
    shots.capture(&app_on(SetupPage::Welcome, Some(ready())), "welcome/W2");

    let finish = [
        ("F1-all-set", Some(ready())),
        ("F2-almost-there", Some(systems::waiting_for_login())),
        (
            "F3-not-finished-resumable",
            Some(systems::service_foreign_kept()),
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
    capture_rows(
        &mut Shots::new(),
        "xremap",
        Step::Xremap,
        e2e::xremap_rows(),
    );
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn keyboard_access() {
    capture_rows(
        &mut Shots::new(),
        "group",
        Step::InputGroup,
        e2e::keyboard_access_rows(),
    );
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn virtual_keyboard() {
    capture_rows(
        &mut Shots::new(),
        "uinput",
        Step::Uinput,
        e2e::virtual_keyboard_rows(),
    );
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn service() {
    capture_rows(
        &mut Shots::new(),
        "service",
        Step::Service,
        e2e::service_rows(),
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
        ("T1a-installing", app_on_step(Step::Xremap, systems::xremap_missing(), busy(Step::Xremap))),
        ("T1b-updating", app_on_step(Step::Xremap, systems::xremap_outdated(), busy(Step::Xremap))),
        ("T1c-group-approval", app_on_step(Step::InputGroup, systems::not_member(), busy(Step::InputGroup))),
        ("T1d-uinput-approval", app_on_step(Step::Uinput, systems::uinput_missing(), busy(Step::Uinput))),
        ("T1e-turning-on", app_on_step(Step::Service, systems::service_missing(), busy(Step::Service))),
        ("T1f-replacing", app_on_step(Step::Service, systems::service_foreign_kept(), busy(Step::Service))),
        ("T1g-updating-service", app_on_step(Step::Service, systems::service_stale(), busy(Step::Service))),
        ("T1h-starting", app_on_step(Step::Service, systems::service_foreign_stopped(), busy(Step::Service))),
        (
            "T2-cancelled",
            app_on_step(Step::InputGroup, systems::not_member(), failed(Step::InputGroup, ActionError::Cancelled)),
        ),
        (
            "T3-not-authorized",
            app_on_step(Step::Uinput, systems::uinput_missing(), failed(Step::Uinput, ActionError::NotAuthorized)),
        ),
        (
            "T4-no-polkit",
            app_on_step(Step::InputGroup, systems::not_member(), |s| {
                s.error = Some((Step::InputGroup, ActionError::NoPolkit));
                s.details = true;
                s.details_for_failure = true;
            }),
        ),
        (
            "T5a-download-failed",
            app_on_step(
                Step::Xremap,
                systems::xremap_missing(),
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
                systems::service_missing(),
                failed(
                    Step::Service,
                    ActionError::Failed(
                        "Job for xremap.service failed because the control process exited with error code"
                            .to_owned(),
                    ),
                ),
            ),
        ),
        ("T6a-details-save-file", app_on_step(Step::Service, systems::service_missing(), details)),
        ("T6b-details-commands", app_on_step(Step::InputGroup, systems::not_member(), details)),
        ("T6c-details-installed", app_on_step(Step::Xremap, ready(), details)),
        (
            "T8-copied",
            app_on_step(Step::InputGroup, systems::not_member(), |s| {
                s.details = true;
                s.copied = true;
            }),
        ),
        (
            "T9-saving",
            app_on_step(Step::Service, systems::service_missing(), |s| {
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
        Some(systems::service_foreign_kept()),
    );
    shots.capture_as(&replace, "variations/C2b-light-replace", &light, &WINDOW);

    let replace_details = app_on_step(Step::Service, systems::service_foreign_kept(), |s| {
        s.details = true
    });
    shots.capture_as(
        &replace_details,
        "variations/C3a-min-window-replace-details",
        &cosmic::Theme::dark(),
        &MIN_WINDOW,
    );
    let finish = app_on(SetupPage::Finish, Some(systems::service_foreign_kept()));
    shots.capture_as(
        &finish,
        "variations/C3b-min-window-finish",
        &cosmic::Theme::dark(),
        &MIN_WINDOW,
    );

    let long_paths = app_on_step(Step::Service, systems::long_paths(), |s| s.details = true);
    shots.capture(&long_paths, "variations/C4-long-paths");
}

// ---- Decks ---------------------------------------------------------------

/// A recognised keyboard as the monitor reports it, for staging its
/// deck without a keyboard attached.
fn recognised(
    name: &str,
    product: u16,
    variant: known::Variant,
    driver: Option<known::AppleDriver>,
) -> monitor::KeyboardDevice {
    let keyboard =
        known::identify(known::APPLE_BLUETOOTH, product, name).expect("a listed product id");
    monitor::KeyboardDevice {
        path: PathBuf::from(DECK_DEVICE),
        id: monitor::KeyboardId::new(
            evdev::InputId::new(
                evdev::BusType::BUS_BLUETOOTH,
                known::APPLE_BLUETOOTH,
                product,
                1,
            ),
            Some(name),
            None,
            name,
        ),
        name: name.to_owned(),
        connected: true,
        form: keyboard.form,
        form_hinted: true,
        iso: variant.is_iso(),
        virtual_device: false,
        known: Some(known::Recognized {
            keyboard,
            variant,
            apple_driver: driver,
        }),
    }
}

/// The event node the staged keyboards sit on.
const DECK_DEVICE: &str = "/dev/input/event20";

/// The application showing a recognised keyboard's deck.
fn app_showing(device: monitor::KeyboardDevice) -> App {
    let mut app = staging::app();
    let _ = app.update(Message::Monitor(monitor::Event::Started(vec![device])));
    let _ = app.update(Message::SelectDevice(DECK_DEVICE.to_owned()));
    app
}

/// The Apple decks of `docs/Supported_Keyboards.md`: each generation
/// in the keyboard view, the function row with F keys first, and the
/// tester naming a held key as printed.
#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn decks() {
    let mut shots = Shots::new();
    let models: [(&str, &str, u16, known::Variant); 6] = [
        (
            "D1-touch-id-2021",
            "Blake’s Magic Keyboard",
            0x029a,
            known::Variant::Ansi,
        ),
        (
            "D2-touch-id-2021-iso",
            "Magic Keyboard",
            0x029a,
            known::Variant::Iso,
        ),
        (
            "D3-lock-2024",
            "Magic Keyboard",
            0x0320,
            known::Variant::Ansi,
        ),
        (
            "D4-numeric-keypad-2017",
            "Magic Keyboard with Numeric Keypad",
            0x026c,
            known::Variant::Ansi,
        ),
        (
            "D5-touch-id-numeric-keypad-2021",
            "Magic Keyboard with Touch ID and Numeric Keypad",
            0x029f,
            known::Variant::Ansi,
        ),
        (
            "D6-touch-id-numeric-keypad-2024",
            "Magic Keyboard with Touch ID and Numeric Keypad",
            0x0322,
            known::Variant::Ansi,
        ),
    ];
    for (id, name, product, variant) in models {
        let app = app_showing(recognised(
            name,
            product,
            variant,
            Some(known::AppleDriver::default()),
        ));
        shots.capture(&app, &format!("decks/{id}"));
    }

    let f_keys_first = known::AppleDriver {
        fnmode: 2,
        ..known::AppleDriver::default()
    };
    let mut app = app_showing(recognised(
        "Blake’s Magic Keyboard",
        0x029a,
        known::Variant::Ansi,
        Some(f_keys_first),
    ));
    shots.capture(&app, "decks/D7-f-keys-first");

    let _ = app.update(Message::SetView(View::Tester));
    let _ = app.update(Message::Monitor(monitor::Event::Key {
        device: PathBuf::from(DECK_DEVICE),
        event: monitor::KeyEvent::Pressed(evdev::KeyCode::KEY_LEFTMETA.0),
    }));
    shots.capture(&app, "decks/D8-tester-command-held");
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

/// A recorder for a flow's driver: each frame the flow names becomes
/// the next image of the storyboard.
fn record(id: &'static str) -> Recorder {
    let mut board = Storyboard::new(id);
    Some(Box::new(move |app: &App, label: &str| {
        board.frame(app, label);
    }))
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb1_fresh_system() {
    e2e::storyboards::fresh_system(record("SB1-fresh-system"));
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb2_foreign_unit_kept() {
    e2e::storyboards::foreign_unit_kept(record("SB2-foreign-unit-kept"));
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb3_authorization_refused_then_granted() {
    e2e::storyboards::authorization_refused_then_granted(record("SB3-authorization-refused"));
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb4_no_polkit() {
    e2e::storyboards::no_polkit(record("SB4-no-polkit"));
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb5_reopened_after_completion() {
    e2e::storyboards::reopened_after_completion(record("SB5-reopened"));
}

#[test]
#[ignore = "writes PNGs under target/setup-shots; run with --ignored"]
fn storyboard_sb6_download_out_of_date() {
    e2e::storyboards::download_out_of_date(record("SB6-download-out-of-date"));
}

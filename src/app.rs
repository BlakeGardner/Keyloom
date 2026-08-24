//! The COSMIC application: state, update logic, and the keyboard view.

use std::collections::HashSet;

use cosmic::app::{Core, Task};
use cosmic::iced::futures::{Stream, StreamExt};
use cosmic::iced::{Alignment, Font, Length, Subscription};
use cosmic::prelude::*;
use cosmic::{theme, widget};

use crate::keyboard::{self, Cap, FORM_FACTORS, Key, KeyDef, LAYOUTS};
use crate::monitor;

/// Fixed height of the typed-text preview area.
const PREVIEW_HEIGHT: f32 = 96.0;

/// Messages emitted by the UI.
#[derive(Clone, Debug)]
pub enum Message {
    KeyPressed(Key),
    LayoutSelected(usize),
    FormSelected(usize),
    Clear,
    Monitor(monitor::Event),
}

/// The on-screen keyboard application.
pub struct App {
    core: Core,
    /// Index of the active layout in [`LAYOUTS`].
    layout: usize,
    /// Index of the active form factor in [`FORM_FACTORS`].
    form: usize,
    /// Key rows of the active layout on the active form factor.
    rows: Vec<Vec<Cap>>,
    /// Layout names shown by the header dropdown.
    layout_names: Vec<&'static str>,
    /// Form factor names shown by the header dropdown.
    form_names: Vec<&'static str>,
    /// Whether the active layout was auto-detected from system settings
    /// (cleared once the user picks one manually).
    auto_detected: bool,
    /// Whether the active form factor was detected from the connected
    /// keyboards (cleared once the user picks one manually).
    form_detected: bool,
    /// Whether the user explicitly picked a size, which stops later
    /// detection from overriding it.
    form_chosen: bool,
    /// Text "typed" so far by clicking the virtual keys.
    typed: String,
    /// One-shot shift toggled by clicking a Shift cap (cleared after the
    /// next character).
    shift: bool,
    /// One-shot AltGr toggled by clicking the AltGr cap (cleared after the
    /// next character).
    altgr: bool,
    caps: bool,
    ctrl: bool,
    alt: bool,
    super_key: bool,
    /// Scancodes of physical keys currently held down.
    held: HashSet<u16>,
    /// Number of keyboards being monitored, once the watcher reports in.
    monitored: Option<usize>,
}

impl App {
    /// Effective shift state: one-shot click shift or a physically held Shift.
    fn shift_active(&self) -> bool {
        self.shift || self.key_held(Key::Shift)
    }

    /// Effective AltGr state: one-shot click AltGr or a physically held AltGr.
    fn altgr_active(&self) -> bool {
        self.altgr || self.key_held(Key::AltGr)
    }

    /// Look up the active layout's key for an evdev scancode.
    fn key_for_code(&self, code: u16) -> Option<Key> {
        self.rows
            .iter()
            .flat_map(|row| row.iter())
            .find_map(|cap| match cap {
                Cap::Key(def) if def.code == code => Some(def.key),
                _ => None,
            })
    }

    /// Rebuild the key rows after a layout or form factor change.
    fn rebuild_rows(&mut self) {
        self.rows = LAYOUTS[self.layout].rows(&FORM_FACTORS[self.form]);
    }

    /// Whether any physically held scancode maps to this layout key.
    fn key_held(&self, key: Key) -> bool {
        self.held
            .iter()
            .any(|code| self.key_for_code(*code) == Some(key))
    }

    /// Append a character, honoring AltGr, shift, and caps lock.
    fn push_char(&mut self, lower: char, upper: char, altgr: Option<char>) {
        let shifted = self.shift_active() != (self.caps && lower.is_alphabetic());
        let base = if shifted { upper } else { lower };
        let altgr = altgr.filter(|_| self.altgr_active());

        self.typed.push(altgr.unwrap_or(base));
        self.shift = false;
        self.altgr = false;
    }

    /// Apply a clicked virtual key to the app state.
    fn press(&mut self, key: Key) {
        match key {
            Key::Char {
                lower,
                upper,
                altgr,
            } => self.push_char(lower, upper, altgr),
            Key::Space => {
                self.typed.push(' ');
                self.shift = false;
                self.altgr = false;
            }
            Key::Tab => self.typed.push('\t'),
            Key::Enter => self.typed.push('\n'),
            Key::Backspace => {
                self.typed.pop();
            }
            Key::Shift => self.shift = !self.shift,
            Key::AltGr => self.altgr = !self.altgr,
            Key::CapsLock => self.caps = !self.caps,
            Key::Ctrl => self.ctrl = !self.ctrl,
            Key::Alt => self.alt = !self.alt,
            Key::Super => self.super_key = !self.super_key,
            // Display-only keys (F-row, navigation, …) don't type anything.
            Key::Named(_) => {}
        }
    }

    /// Apply a physical key press (or autorepeat) observed via evdev.
    ///
    /// Unlike clicks, physical modifiers don't toggle: their state is
    /// tracked while held via [`Self::held`]. Caps Lock still latches.
    fn phys_press(&mut self, code: u16, repeat: bool) {
        let Some(key) = self.key_for_code(code) else {
            return;
        };

        match key {
            Key::Char {
                lower,
                upper,
                altgr,
            } => self.push_char(lower, upper, altgr),
            Key::Space => self.typed.push(' '),
            Key::Tab => self.typed.push('\t'),
            Key::Enter => self.typed.push('\n'),
            Key::Backspace => {
                self.typed.pop();
            }
            Key::CapsLock if !repeat => self.caps = !self.caps,
            _ => {}
        }
    }

    /// Whether a key should be rendered in its highlighted (active) state.
    fn is_active(&self, key: Key) -> bool {
        match key {
            Key::Shift => self.shift_active(),
            Key::AltGr => self.altgr_active(),
            Key::CapsLock => self.caps,
            Key::Ctrl => self.ctrl,
            Key::Alt => self.alt,
            Key::Super => self.super_key,
            _ => false,
        }
    }

    /// The label shown on a key cap, given the current modifier state.
    fn label(&self, key: Key) -> String {
        match key {
            Key::Char {
                lower,
                upper,
                altgr,
            } => {
                let shifted = self.shift_active() != (self.caps && lower.is_alphabetic());
                let altgr = altgr.filter(|_| self.altgr_active());
                altgr.unwrap_or(if shifted { upper } else { lower }).to_string()
            }
            Key::Backspace => "⌫".into(),
            Key::Tab => "Tab ⇥".into(),
            Key::CapsLock => "Caps ⇪".into(),
            Key::Enter => "Enter ⏎".into(),
            Key::Shift => "⇧ Shift".into(),
            Key::Ctrl => "Ctrl".into(),
            Key::Super => "Super".into(),
            Key::Alt => "Alt".into(),
            Key::AltGr => "AltGr".into(),
            Key::Space => String::new(),
            Key::Named(name) => name.into(),
        }
    }

    /// Build one clickable key cap.
    fn key_button(&self, def: &KeyDef) -> Element<'_, Message> {
        // Highlight when logically active (modifiers) or physically held
        // (this exact key, so left/right modifiers depress separately).
        let class = if self.is_active(def.key) || self.held.contains(&def.code) {
            theme::Button::Suggested
        } else {
            theme::Button::Standard
        };

        let label = def
            .label
            .map_or_else(|| self.label(def.key), str::to_owned);

        widget::button::custom(
            widget::text(label)
                .size(16.0)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .class(class)
        .width(Length::FillPortion(def.width))
        .height(Length::Fill)
        .on_press_down(Message::KeyPressed(def.key))
        .into()
    }
}

impl cosmic::Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "com.example.CosmicKeyboard";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, (): Self::Flags) -> (Self, Task<Message>) {
        let detected = keyboard::detect();
        let layout = detected.unwrap_or(0);

        match detected {
            Some(index) => eprintln!("auto-detected keyboard layout: {}", LAYOUTS[index].name),
            None => eprintln!(
                "could not detect a supported system layout; defaulting to {}",
                LAYOUTS[layout].name
            ),
        }

        let form = keyboard::FORM_FULL;

        let mut app = App {
            core,
            layout,
            form,
            rows: LAYOUTS[layout].rows(&FORM_FACTORS[form]),
            layout_names: LAYOUTS.iter().map(|layout| layout.name).collect(),
            form_names: FORM_FACTORS.iter().map(|form| form.name).collect(),
            auto_detected: detected.is_some(),
            form_detected: false,
            form_chosen: false,
            typed: String::new(),
            shift: false,
            altgr: false,
            caps: false,
            ctrl: false,
            alt: false,
            super_key: false,
            held: HashSet::new(),
            monitored: None,
        };

        app.set_header_title("Virtual Keyboard".to_owned());

        (app, Task::none())
    }

    fn header_end(&self) -> Vec<Element<'_, Message>> {
        vec![
            widget::dropdown(
                self.form_names.as_slice(),
                Some(self.form),
                Message::FormSelected,
            )
            .into(),
            widget::dropdown(
                self.layout_names.as_slice(),
                Some(self.layout),
                Message::LayoutSelected,
            )
            .into(),
            widget::button::standard("Clear")
                .on_press(Message::Clear)
                .into(),
        ]
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::KeyPressed(key) => self.press(key),
            Message::LayoutSelected(index) => {
                if index < LAYOUTS.len() && index != self.layout {
                    self.layout = index;
                    self.auto_detected = false;
                    self.rebuild_rows();
                }
            }
            Message::FormSelected(index) => {
                if index < FORM_FACTORS.len() {
                    self.form_chosen = true;

                    if index != self.form {
                        self.form = index;
                        self.form_detected = false;
                        self.rebuild_rows();
                    }
                }
            }
            Message::Clear => self.typed.clear(),
            Message::Monitor(event) => match event {
                monitor::Event::Started { devices, form } => {
                    self.monitored = Some(devices);

                    // Adopt the detected size unless the user already chose.
                    if let Some(index) = form
                        && !self.form_chosen
                    {
                        self.form_detected = true;

                        if index != self.form {
                            self.form = index;
                            self.rebuild_rows();
                        }
                    }
                }
                monitor::Event::Key(monitor::KeyEvent::Pressed(code)) => {
                    self.held.insert(code);
                    self.phys_press(code, false);
                }
                monitor::Event::Key(monitor::KeyEvent::Repeated(code)) => {
                    self.phys_press(code, true);
                }
                monitor::Event::Key(monitor::KeyEvent::Released(code)) => {
                    self.held.remove(&code);
                }
            },
        }

        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::run(monitor_stream)
    }

    fn view(&self) -> Element<'_, Message> {
        let spacing = theme::spacing();

        // Preview area showing what has been "typed" so far.
        let preview = widget::container(
            widget::text(format!("{}▏", self.typed))
                .font(Font::MONOSPACE)
                .size(18.0),
        )
        .class(theme::Container::Card)
        .padding(spacing.space_s)
        .width(Length::Fill)
        .height(Length::Fixed(PREVIEW_HEIGHT));

        let status = match self.monitored {
            None => widget::text::caption("Starting keyboard monitor…".to_owned()),
            Some(0) => widget::text::caption(
                "No readable keyboards in /dev/input — add your user to the “input” group \
                 to mirror physical typing."
                    .to_owned(),
            ),
            Some(n) => {
                widget::text::caption(format!("Mirroring {n} physical keyboards via evdev"))
            }
        };

        let auto = |detected: bool| if detected { " (auto)" } else { "" };

        let layout_note = widget::text::caption(format!(
            "{}{} · {}{}",
            FORM_FACTORS[self.form].name,
            auto(self.form_detected),
            LAYOUTS[self.layout].name,
            auto(self.auto_detected),
        ));

        let status_row = widget::row::with_capacity(2)
            .push(status.width(Length::Fill))
            .push(layout_note);

        let mut content = widget::column::with_capacity(self.rows.len() + 2)
            .spacing(spacing.space_xxs)
            .padding(spacing.space_xs);

        content = content.push(preview);
        content = content.push(status_row);

        for row in &self.rows {
            let mut keys = widget::row::with_capacity(row.len()).spacing(spacing.space_xxs);

            for cap in row {
                keys = keys.push(match cap {
                    Cap::Key(def) => self.key_button(def),
                    Cap::Gap(width) => widget::Space::new()
                        .width(Length::FillPortion(*width))
                        .height(Length::Fill)
                        .into(),
                });
            }

            content = content.push(keys.width(Length::Fill).height(Length::Fill));
        }

        content.into()
    }
}

/// Adapts the evdev watcher into this app's message stream.
fn monitor_stream() -> impl Stream<Item = Message> + Send {
    monitor::watch().map(Message::Monitor)
}

//! The COSMIC application: state, update logic, and the keyboard view.

use std::collections::HashSet;

use cosmic::app::{Core, Task};
use cosmic::iced::futures::{Stream, StreamExt};
use cosmic::iced::{Alignment, Font, Length, Subscription};
use cosmic::prelude::*;
use cosmic::{theme, widget};

use crate::keyboard::{Key, KeyDef, ROWS, key_for_code};
use crate::monitor;

/// Fixed height of the typed-text preview area.
const PREVIEW_HEIGHT: f32 = 96.0;

/// Messages emitted by the UI.
#[derive(Clone, Debug)]
pub enum Message {
    KeyPressed(Key),
    Clear,
    Monitor(monitor::Event),
}

/// The on-screen keyboard application.
pub struct App {
    core: Core,
    /// Text "typed" so far by clicking the virtual keys.
    typed: String,
    /// One-shot shift toggled by clicking a Shift cap (cleared after the
    /// next character).
    shift: bool,
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

    /// Whether any physically held scancode maps to this layout key.
    fn key_held(&self, key: Key) -> bool {
        self.held
            .iter()
            .any(|code| key_for_code(*code) == Some(key))
    }

    /// Append a character, honoring shift and caps lock.
    fn push_char(&mut self, lower: char, upper: char) {
        let shifted = self.shift_active() != (self.caps && lower.is_ascii_alphabetic());
        self.typed.push(if shifted { upper } else { lower });
        self.shift = false;
    }

    /// Apply a clicked virtual key to the app state.
    fn press(&mut self, key: Key) {
        match key {
            Key::Char { lower, upper } => self.push_char(lower, upper),
            Key::Space => {
                self.typed.push(' ');
                self.shift = false;
            }
            Key::Tab => self.typed.push('\t'),
            Key::Enter => self.typed.push('\n'),
            Key::Backspace => {
                self.typed.pop();
            }
            Key::Shift => self.shift = !self.shift,
            Key::CapsLock => self.caps = !self.caps,
            Key::Ctrl => self.ctrl = !self.ctrl,
            Key::Alt => self.alt = !self.alt,
            Key::Super => self.super_key = !self.super_key,
        }
    }

    /// Apply a physical key press (or autorepeat) observed via evdev.
    ///
    /// Unlike clicks, physical modifiers don't toggle: their state is
    /// tracked while held via [`Self::held`]. Caps Lock still latches.
    fn phys_press(&mut self, code: u16, repeat: bool) {
        let Some(key) = key_for_code(code) else {
            return;
        };

        match key {
            Key::Char { lower, upper } => self.push_char(lower, upper),
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
            Key::Char { lower, upper } => {
                let shifted = self.shift_active() != (self.caps && lower.is_ascii_alphabetic());
                (if shifted { upper } else { lower }).to_string()
            }
            Key::Backspace => "⌫".into(),
            Key::Tab => "Tab ⇥".into(),
            Key::CapsLock => "Caps ⇪".into(),
            Key::Enter => "Enter ⏎".into(),
            Key::Shift => "⇧ Shift".into(),
            Key::Ctrl => "Ctrl".into(),
            Key::Super => "Super".into(),
            Key::Alt => "Alt".into(),
            Key::Space => String::new(),
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

        widget::button::custom(
            widget::text(self.label(def.key))
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
        let mut app = App {
            core,
            typed: String::new(),
            shift: false,
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
            widget::button::standard("Clear")
                .on_press(Message::Clear)
                .into(),
        ]
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::KeyPressed(key) => self.press(key),
            Message::Clear => self.typed.clear(),
            Message::Monitor(event) => match event {
                monitor::Event::Started { devices } => self.monitored = Some(devices),
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

        let mut content = widget::column::with_capacity(ROWS.len() + 2)
            .spacing(spacing.space_xxs)
            .padding(spacing.space_xs);

        content = content.push(preview);
        content = content.push(status);

        for row in ROWS {
            let mut keys = widget::row::with_capacity(row.len()).spacing(spacing.space_xxs);

            for def in *row {
                keys = keys.push(self.key_button(def));
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

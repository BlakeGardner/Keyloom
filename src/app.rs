//! The COSMIC application: state, update logic, and the keyboard view.

use cosmic::app::{Core, Task};
use cosmic::iced::{Alignment, Font, Length};
use cosmic::prelude::*;
use cosmic::{theme, widget};

use crate::keyboard::{Key, KeyDef, ROWS};

/// Fixed height of the typed-text preview area.
const PREVIEW_HEIGHT: f32 = 96.0;

/// Messages emitted by the UI.
#[derive(Clone, Debug)]
pub enum Message {
    KeyPressed(Key),
    Clear,
}

/// The on-screen keyboard application.
pub struct App {
    core: Core,
    /// Text "typed" so far by clicking the virtual keys.
    typed: String,
    /// One-shot shift (cleared after the next character).
    shift: bool,
    caps: bool,
    ctrl: bool,
    alt: bool,
    super_key: bool,
}

impl App {
    /// Apply a virtual key press to the app state.
    ///
    /// This only edits the local preview buffer for now; sending real
    /// input events to the compositor can be hooked in here later.
    fn press(&mut self, key: Key) {
        match key {
            Key::Char { lower, upper } => {
                let shifted = self.shift != (self.caps && lower.is_ascii_alphabetic());
                self.typed.push(if shifted { upper } else { lower });
                self.shift = false;
            }
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

    /// Whether a key should be rendered in its highlighted (active) state.
    fn is_active(&self, key: Key) -> bool {
        match key {
            Key::Shift => self.shift,
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
                let shifted = self.shift != (self.caps && lower.is_ascii_alphabetic());
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
        let class = if self.is_active(def.key) {
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
        }

        Task::none()
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

        let mut content = widget::column::with_capacity(ROWS.len() + 1)
            .spacing(spacing.space_xxs)
            .padding(spacing.space_xs);

        content = content.push(preview);

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

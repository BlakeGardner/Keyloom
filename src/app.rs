//! The application: state, update logic, and the keyboard view.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
    DeviceSelected(usize),
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
    /// Physical keys held down, including their source to distinguish overlaps.
    held: HashSet<(PathBuf, u16)>,
    /// Readable keyboards discovered by the monitor at startup.
    devices: Vec<monitor::KeyboardDevice>,
    device_names: Vec<String>,
    /// Dropdown index: zero mirrors all keyboards, subsequent entries one device.
    selected_device: usize,
    monitor_started: bool,
}

impl App {
    fn selected_keyboard(&self) -> Option<&monitor::KeyboardDevice> {
        self.selected_device
            .checked_sub(1)
            .and_then(|index| self.devices.get(index))
    }

    fn accepts_device(&self, path: &Path) -> bool {
        self.devices
            .iter()
            .any(|device| device.connected && device.path == path)
            && self
                .selected_keyboard()
                .is_none_or(|device| device.path == path)
    }

    fn refresh_device_names(&mut self) {
        self.device_names = std::iter::once("All keyboards".to_owned())
            .chain(self.devices.iter().map(|device| {
                let mut name = device.name.clone();
                if self
                    .devices
                    .iter()
                    .filter(|other| other.name == device.name)
                    .count()
                    > 1
                {
                    // Identical hardware names still need distinct list entries.
                    name.push_str(&format!(" ({})", device.path.display()));
                }
                if !device.connected {
                    name.push_str(" (disconnected)");
                }
                name
            }))
            .collect();
    }

    fn detect_form(&mut self) {
        if self.form_chosen {
            return;
        }

        let form = monitor::detected_form(self.devices.iter().enumerate().filter_map(
            |(index, device)| {
                (self.selected_device == 0 || self.selected_device == index + 1).then_some(device)
            },
        ));
        self.form_detected = form.is_some();
        if let Some(index) = form
            && index != self.form
        {
            self.form = index;
            self.rebuild_rows();
        }
    }

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
            .any(|(_, code)| self.key_for_code(*code) == Some(key))
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
                altgr
                    .unwrap_or(if shifted { upper } else { lower })
                    .to_string()
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
        let class =
            if self.is_active(def.key) || self.held.iter().any(|(_, code)| *code == def.code) {
                theme::Button::Suggested
            } else {
                theme::Button::Standard
            };

        let label = def.label.map_or_else(|| self.label(def.key), str::to_owned);

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

    const APP_ID: &'static str = "io.github.blakegardner.Keyloom";

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
            devices: Vec::new(),
            device_names: vec!["All keyboards".to_owned()],
            selected_device: 0,
            monitor_started: false,
        };

        app.set_header_title("Keyloom".to_owned());

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
                    self.form_detected = false;

                    if index != self.form {
                        self.form = index;
                        self.rebuild_rows();
                    }
                }
            }
            Message::DeviceSelected(index) => {
                if index <= self.devices.len() && index != self.selected_device {
                    self.selected_device = index;
                    // Releases from the previous source will now be filtered out.
                    self.held.clear();
                    self.detect_form();
                }
            }
            Message::Clear => self.typed.clear(),
            Message::Monitor(event) => match event {
                monitor::Event::Started(devices) => {
                    self.devices = devices;
                    self.selected_device = 0;
                    self.held.clear();
                    self.monitor_started = true;
                    self.refresh_device_names();
                    self.detect_form();
                }
                monitor::Event::Key { device, event } => {
                    if self.accepts_device(&device) {
                        match event {
                            monitor::KeyEvent::Pressed(code) => {
                                self.held.insert((device, code));
                                self.phys_press(code, false);
                            }
                            monitor::KeyEvent::Repeated(code) => {
                                self.phys_press(code, true);
                            }
                            monitor::KeyEvent::Released(code) => {
                                self.held.remove(&(device, code));
                            }
                        }
                    }
                }
                monitor::Event::Disconnected(path) => {
                    if let Some(device) = self.devices.iter_mut().find(|device| device.path == path)
                    {
                        device.connected = false;
                    }
                    self.held.retain(|(device, _)| *device != path);
                    self.refresh_device_names();
                    self.detect_form();
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

        let input_row = widget::row::with_capacity(2)
            .spacing(spacing.space_s)
            .align_y(Alignment::Center)
            .push(widget::text("Input device"))
            .push(
                widget::dropdown(
                    self.device_names.as_slice(),
                    Some(self.selected_device),
                    Message::DeviceSelected,
                )
                .width(Length::Fill),
            );

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

        let status = widget::text::caption(if !self.monitor_started {
            "Starting keyboard monitor…".to_owned()
        } else if let Some(device) = self.selected_keyboard() {
            if device.connected {
                format!("Mirroring {}", device.name)
            } else {
                "Selected keyboard disconnected — choose another input device or restart to rescan."
                    .to_owned()
            }
        } else {
            match self
                .devices
                .iter()
                .filter(|device| device.connected)
                .count()
            {
                0 if !self.devices.is_empty() => {
                    "No connected keyboards — restart to rescan input devices.".to_owned()
                }
                0 => "No readable keyboards in /dev/input — add your user to the “input” group \
                 to mirror physical typing."
                    .to_owned(),
                1 => "Mirroring 1 keyboard".to_owned(),
                n => format!("Mirroring all {n} keyboards"),
            }
        });

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

        let mut content = widget::column::with_capacity(self.rows.len() + 3)
            .spacing(spacing.space_xxs)
            .padding(spacing.space_xs);

        content = content.push(input_row);
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

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::Application;
    use evdev::KeyCode;
    use monitor::{Event, KeyEvent, KeyboardDevice};

    fn device(path: &str, form: usize, form_hinted: bool) -> KeyboardDevice {
        KeyboardDevice {
            path: path.into(),
            name: "Test keyboard".to_owned(),
            connected: true,
            form,
            form_hinted,
        }
    }

    fn app_with_devices(devices: Vec<KeyboardDevice>) -> App {
        let (mut app, _) = App::init(Core::default(), ());
        app.layout = 0; // Use US labels regardless of the machine's configuration.
        app.rebuild_rows();
        let _ = app.update(Message::Monitor(Event::Started(devices)));
        app
    }

    fn two_keyboards() -> App {
        app_with_devices(vec![
            device("/dev/input/event1", keyboard::FORM_FULL, false),
            device("/dev/input/event2", keyboard::FORM_SIXTY, true),
        ])
    }

    fn key(app: &mut App, path: &str, event: KeyEvent) {
        let _ = app.update(Message::Monitor(Event::Key {
            device: path.into(),
            event,
        }));
    }

    #[test]
    fn selected_device_filters_presses_repeats_and_releases() {
        let mut app = two_keyboards();
        let _ = app.update(Message::DeviceSelected(1));
        let a = KeyCode::KEY_A.0;
        let shift = KeyCode::KEY_LEFTSHIFT.0;

        key(&mut app, "/dev/input/event2", KeyEvent::Pressed(shift));
        key(&mut app, "/dev/input/event2", KeyEvent::Pressed(a));
        key(&mut app, "/dev/input/event2", KeyEvent::Repeated(a));
        assert!(app.typed.is_empty());
        assert!(app.held.is_empty());

        key(&mut app, "/dev/input/event1", KeyEvent::Pressed(a));
        key(&mut app, "/dev/input/event1", KeyEvent::Repeated(a));
        key(&mut app, "/dev/input/event2", KeyEvent::Released(a));
        assert_eq!(app.typed, "aa");
        assert_eq!(app.held.len(), 1);
        key(&mut app, "/dev/input/event1", KeyEvent::Released(a));
        assert!(app.held.is_empty());
    }

    #[test]
    fn switching_sources_clears_held_modifiers_and_preserves_preview() {
        let mut app = two_keyboards();
        let shift = KeyCode::KEY_LEFTSHIFT.0;
        let a = KeyCode::KEY_A.0;
        key(&mut app, "/dev/input/event1", KeyEvent::Pressed(shift));
        key(&mut app, "/dev/input/event1", KeyEvent::Pressed(a));
        assert_eq!(app.typed, "A");

        let _ = app.update(Message::DeviceSelected(2));
        assert!(!app.shift_active());
        assert!(app.held.is_empty());
        key(&mut app, "/dev/input/event2", KeyEvent::Pressed(a));
        key(&mut app, "/dev/input/event1", KeyEvent::Pressed(a));
        assert_eq!(app.typed, "Aa");

        let _ = app.update(Message::DeviceSelected(0));
        key(&mut app, "/dev/input/event1", KeyEvent::Pressed(a));
        key(&mut app, "/dev/input/event2", KeyEvent::Pressed(a));
        assert_eq!(app.typed, "Aaaa");
    }

    #[test]
    fn all_keyboards_keep_overlapping_keys_held_until_each_releases() {
        let mut app = two_keyboards();
        let shift = KeyCode::KEY_LEFTSHIFT.0;
        key(&mut app, "/dev/input/event1", KeyEvent::Pressed(shift));
        key(&mut app, "/dev/input/event2", KeyEvent::Pressed(shift));
        key(&mut app, "/dev/input/event1", KeyEvent::Released(shift));
        assert!(app.shift_active());
        key(&mut app, "/dev/input/event2", KeyEvent::Released(shift));
        assert!(!app.shift_active());
    }

    #[test]
    fn form_follows_selected_keyboard_until_manually_chosen() {
        let mut app = two_keyboards();
        // In aggregate mode, the named size hint wins over generic capabilities.
        assert_eq!(app.form, keyboard::FORM_SIXTY);
        assert!(app.form_detected);
        let _ = app.update(Message::DeviceSelected(1));
        assert_eq!(app.form, keyboard::FORM_FULL);
        let _ = app.update(Message::DeviceSelected(2));
        assert_eq!(app.form, keyboard::FORM_SIXTY);

        // Explicitly choosing even the current size disables later detection.
        let _ = app.update(Message::FormSelected(keyboard::FORM_SIXTY));
        let _ = app.update(Message::DeviceSelected(1));
        assert_eq!(app.form, keyboard::FORM_SIXTY);
        assert!(!app.form_detected);
    }

    #[test]
    fn duplicate_names_remain_distinct_and_disconnection_keeps_selection() {
        let mut app = two_keyboards();
        assert_ne!(app.device_names[1], app.device_names[2]);
        assert!(app.device_names[1].contains("/dev/input/event1"));
        let _ = app.update(Message::DeviceSelected(1));
        key(
            &mut app,
            "/dev/input/event1",
            KeyEvent::Pressed(KeyCode::KEY_LEFTSHIFT.0),
        );
        let _ = app.update(Message::Monitor(Event::Disconnected(
            "/dev/input/event1".into(),
        )));
        assert_eq!(app.selected_device, 1);
        assert!(app.device_names[1].contains("disconnected"));
        assert!(!app.shift_active());
        assert!(!app.form_detected);
        key(
            &mut app,
            "/dev/input/event2",
            KeyEvent::Pressed(KeyCode::KEY_A.0),
        );
        assert!(app.typed.is_empty());

        let _ = app.update(Message::DeviceSelected(0));
        assert_eq!(app.form, keyboard::FORM_SIXTY);
        key(
            &mut app,
            "/dev/input/event2",
            KeyEvent::Pressed(KeyCode::KEY_A.0),
        );
        assert_eq!(app.typed, "a");
    }

    #[test]
    fn no_readable_devices_still_allows_virtual_typing() {
        let mut app = app_with_devices(Vec::new());
        assert_eq!(app.device_names, ["All keyboards"]);
        let _ = app.update(Message::DeviceSelected(1));
        assert_eq!(app.selected_device, 0);
        let key = app.key_for_code(KeyCode::KEY_A.0).unwrap();
        let _ = app.update(Message::KeyPressed(key));
        assert_eq!(app.typed, "a");
    }
}

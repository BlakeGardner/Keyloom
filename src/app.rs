//! Application state and update logic for the Keyloom GUI.
//!
//! The interface follows the design export in `design/`. Profiles and
//! their mappings persist via cosmic-config, and every change
//! regenerates the xremap configuration written to the user's config
//! directory; shortcut groups are still previewed in memory only.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::Subscription;
use cosmic::iced::futures::{Stream, StreamExt};
use cosmic::prelude::*;

use crate::config::{self, KeyloomConfig};
use crate::keyboard;
use crate::monitor;
use crate::ui;
use crate::ui::model::{self, Chord, Group, Mapping, Maps, Profile, Rule, key_by_evdev, key_name};
use crate::xremap;

/// How long the bottom sheet takes to rise (the design's `kbRise`).
const SHEET_RISE: Duration = Duration::from_millis(220);

/// Main navigation tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Keyboard,
    Tester,
    Shortcuts,
}

/// Which layer the keyboard canvas previews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Base,
    Nav,
}

/// What the key editor's action list assigns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Tap,
    Hold,
    Combo,
}

/// Popovers anchored to the header and device toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Popover {
    Profiles,
    Devices,
    Size,
    Menu,
}

/// Which side of a shortcut rule is being recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    From,
    To,
}

/// Confirmation toast at the bottom of the shell.
#[derive(Clone, Debug)]
pub struct Toast {
    /// Sequence number so delayed dismissals can't remove newer toasts.
    pub id: u64,
    pub text: String,
    pub sub: String,
}

/// Snapshot for the toast's Undo action.
pub enum Undo {
    Maps(HashMap<String, Maps>),
    Groups(HashMap<String, Vec<Group>>),
}

/// The rule opened in the shortcut editor (`rule` is `None` while a
/// newly added rule waits for its first recorded chord).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditRule {
    pub group: usize,
    pub rule: Option<usize>,
}

/// The key most recently observed by the tester.
#[derive(Clone, Debug)]
pub struct LastKey {
    pub code: &'static str,
    pub device: String,
}

/// Messages emitted by the UI.
#[derive(Clone, Debug)]
pub enum Message {
    SetView(View),
    TogglePopover(Popover),
    CloseOverlays,
    SelectProfile(String),
    NewProfile {
        duplicate: bool,
    },
    SelectDevice(String),
    /// Show a specific form factor (size picker; marks it user-chosen).
    SetForm(usize),
    /// Switch between the ANSI and ISO assemblies.
    SetVariant(bool),
    OpenRemaps,
    CloseRemaps,
    SetLayer(Layer),
    ToggleLayers,
    SelectKey(&'static str),
    Query(String),
    SetCategory(&'static str),
    PickAction(String),
    ToggleAdvanced,
    SetMode(Mode),
    ToggleFromMod(usize),
    ToggleToMod(usize),
    RemoveCombo {
        group: usize,
        rule: usize,
    },
    ToggleSwap,
    ClearKey,
    /// Remove one key's mapping from the remaps list.
    RemoveMapping(String),
    ClosePanel,
    SetCapture(bool),
    AddGroup,
    ToggleGroup(usize),
    EditRule {
        group: usize,
        rule: Option<usize>,
    },
    SetRecording(Option<Side>),
    ToggleAnyMod,
    DeleteRule,
    CloseEdit,
    Undo,
    ToastExpired(u64),
    /// Redraw tick while the bottom sheet rises.
    SheetAnimate,
    MenuShowSetup,
    MenuReset,
    MenuAbout,
    SkipOnboarding,
    NextOnboarding,
    Monitor(monitor::Event),
}

/// The Keyloom application.
pub struct App {
    core: Core,
    // Navigation and overlays.
    pub view: View,
    pub popover: Option<Popover>,
    pub toast: Option<Toast>,
    toast_seq: u64,
    pub remaps_open: bool,
    pub onboarding: bool,
    pub onb_step: usize,
    // Profiles and their in-memory preview state.
    pub profiles: Vec<Profile>,
    pub profile: String,
    pub profile_maps: HashMap<String, Maps>,
    pub profile_groups: HashMap<String, Vec<Group>>,
    custom_profiles: usize,
    pub undo: Option<Undo>,
    // Keyboard view state.
    pub device: String,
    /// Displayed form factor (index into [`keyboard::FORM_FACTORS`]).
    pub form: usize,
    /// Whether the deck uses the ISO assembly instead of ANSI.
    pub iso: bool,
    /// Once the user picks a size, detection stops changing it.
    form_overridden: bool,
    pub layer: Layer,
    pub layers_open: bool,
    pub selected: Option<&'static str>,
    pub mode: Mode,
    pub query: String,
    pub category: Option<&'static str>,
    pub advanced: bool,
    pub capture: bool,
    pub from_mods: [bool; 4],
    pub to_mods: [bool; 4],
    // Shortcuts view state.
    pub edit_rule: Option<EditRule>,
    pub recording: Option<Side>,
    /// When the bottom sheet last started opening, for its rise animation.
    sheet_opened: Option<Instant>,
    // Hardware monitoring.
    pub devices: Vec<monitor::KeyboardDevice>,
    pub monitor_started: bool,
    pub pressed: HashSet<(PathBuf, u16)>,
    pub last: Option<LastKey>,
    /// Persistent settings store (None when unavailable or in tests).
    settings: Option<cosmic_config::Config>,
}

impl App {
    /// Mappings of the active profile.
    pub fn maps(&self) -> &Maps {
        static EMPTY: Maps = Vec::new();
        self.profile_maps.get(&self.profile).unwrap_or(&EMPTY)
    }

    /// Look up the active profile's mapping for one key.
    pub fn mapping(&self, code: &str) -> Option<&Mapping> {
        self.maps()
            .iter()
            .find(|(key, _)| key == code)
            .map(|(_, mapping)| mapping)
    }

    /// Shortcut groups of the active profile.
    pub fn groups(&self) -> &[Group] {
        self.profile_groups
            .get(&self.profile)
            .map_or(&[], Vec::as_slice)
    }

    /// Name of the active profile.
    pub fn profile_name(&self) -> &str {
        self.profiles
            .iter()
            .find(|profile| profile.id == self.profile)
            .map_or("Default", |profile| profile.name.as_str())
    }

    /// Display name for a device scope id.
    pub fn device_label(&self, id: &str) -> String {
        if id == "all" {
            return "All keyboards".to_owned();
        }
        if let Some(device) = self
            .devices
            .iter()
            .find(|device| device.path.to_string_lossy() == id)
        {
            return device.name.clone();
        }
        model::DEMO_DEVICES
            .iter()
            .find(|(demo, _, _)| *demo == id)
            .map_or_else(|| id.to_owned(), |(_, name, _)| (*name).to_owned())
    }

    /// The device rows offered by the "Applies to" popover:
    /// `(id, name, sub)`. Real keyboards replace the demo entries as
    /// soon as the monitor reports them.
    pub fn device_entries(&self) -> Vec<(String, String, String)> {
        let mut entries = vec![(
            "all".to_owned(),
            "All keyboards".to_owned(),
            if self.devices.is_empty() {
                "Demo device choices".to_owned()
            } else {
                format!(
                    "{} detected keyboard{}",
                    self.devices.len(),
                    if self.devices.len() == 1 { "" } else { "s" }
                )
            },
        )];
        if self.devices.is_empty() {
            for (id, name, sub) in model::DEMO_DEVICES.iter().skip(1) {
                entries.push(((*id).to_owned(), (*name).to_owned(), (*sub).to_owned()));
            }
        } else {
            for device in &self.devices {
                entries.push((
                    device.path.to_string_lossy().into_owned(),
                    device.name.clone(),
                    if device.connected {
                        device.path.display().to_string()
                    } else {
                        format!("{} (disconnected)", device.path.display())
                    },
                ));
            }
        }
        entries
    }

    /// Whether a key cap is currently held on any monitored keyboard.
    pub fn is_pressed(&self, evdev: u16) -> bool {
        self.pressed.iter().any(|(_, code)| *code == evdev)
    }

    /// The key caps of the currently displayed deck.
    pub fn deck(&self) -> &'static [model::KeyCap] {
        model::deck(self.form, self.iso)
    }

    /// The form factor detection would pick for the connected keyboards.
    pub fn detected_form(&self) -> Option<usize> {
        monitor::detected_form(self.devices.iter())
    }

    /// Held modifiers as `[Ctrl, Shift, Alt, Super]`.
    pub fn held_mods(&self) -> [bool; 4] {
        use evdev::KeyCode as K;
        let held = |codes: &[u16]| {
            codes
                .iter()
                .any(|code| self.pressed.iter().any(|(_, held)| held == code))
        };
        [
            held(&[K::KEY_LEFTCTRL.0, K::KEY_RIGHTCTRL.0]),
            held(&[K::KEY_LEFTSHIFT.0, K::KEY_RIGHTSHIFT.0]),
            held(&[K::KEY_LEFTALT.0, K::KEY_RIGHTALT.0]),
            held(&[K::KEY_LEFTMETA.0, K::KEY_RIGHTMETA.0]),
        ]
    }

    /// Modifier-plus-key shortcut rules that start from the given key
    /// name, as `(group index, rule index, rule)`.
    pub fn combos_for(&self, key: &str) -> Vec<(usize, usize, &Rule)> {
        self.groups()
            .iter()
            .enumerate()
            .flat_map(|(gi, group)| {
                group
                    .rules
                    .iter()
                    .enumerate()
                    .filter(|(_, rule)| rule.from.key == key && !rule.from.mods.is_empty())
                    .map(move |(ri, rule)| (gi, ri, rule))
            })
            .collect()
    }

    /// Whether the bottom editor sheet is showing.
    pub fn sheet_open(&self) -> bool {
        (self.view == View::Keyboard && self.selected.is_some())
            || (self.view == View::Shortcuts && self.edit_rule.is_some())
    }

    /// Linear progress of the sheet's rise animation (1.0 once settled).
    pub fn sheet_progress(&self) -> f32 {
        self.sheet_opened.map_or(1.0, |opened| {
            (opened.elapsed().as_secs_f32() / SHEET_RISE.as_secs_f32()).min(1.0)
        })
    }

    /// Show a confirmation toast (kept until dismissed or replaced),
    /// returning its id for timed dismissal.
    fn flash(&mut self, text: impl Into<String>, sub: impl Into<String>) -> u64 {
        self.toast_seq += 1;
        self.toast = Some(Toast {
            id: self.toast_seq,
            text: text.into(),
            sub: sub.into(),
        });
        self.toast_seq
    }

    /// Save the configuration model and regenerate the xremap file.
    /// Called after every change to profiles or their mappings.
    fn persist(&mut self) {
        // Tests exercise the update loop; never touch the real
        // ~/.config from them.
        if cfg!(test) {
            return;
        }
        if let Some(settings) = &self.settings {
            let snapshot = KeyloomConfig::snapshot(
                &self.profiles,
                &self.profile_maps,
                &self.profile,
                u32::try_from(self.custom_profiles).unwrap_or(u32::MAX),
            );
            if let Err(err) = snapshot.write_entry(settings) {
                eprintln!("keyloom: failed to save settings: {err}");
            }
        }
        self.write_xremap(true);
    }

    /// Regenerate the xremap YAML from the active profile's mappings
    /// and write it to the user's config directory.
    ///
    /// A pre-existing config Keyloom did not generate is backed up and
    /// replaced only when `overwrite_foreign` is set (a user edit);
    /// startup leaves foreign files alone.
    fn write_xremap(&mut self, overwrite_foreign: bool) {
        let yaml = xremap::generate(self.maps(), |id| self.device_label(id));
        match xremap::write(&yaml, overwrite_foreign) {
            Ok(xremap::WriteOutcome::SkippedForeign(path)) => {
                eprintln!(
                    "keyloom: leaving existing xremap config untouched: {}",
                    path.display()
                );
            }
            Ok(_) => {}
            Err(err) => {
                self.flash("Could not write xremap config", err.to_string());
            }
        }
    }

    /// Assign an action to the selected key's tap or hold slot.
    fn set_mapping(&mut self, code: &str, action: &str) {
        if self.view == View::Tester {
            return;
        }
        // Reject a no-op self-mapping. Assigning the key's own name is
        // only meaningful when the other slot changes behavior (e.g.
        // hold → Control with tap kept as the key itself).
        let name = key_name(code);
        if action == name {
            let other = self.mapping(code).and_then(|mapping| {
                if self.mode == Mode::Hold {
                    mapping.tap.clone()
                } else {
                    mapping.hold.clone()
                }
            });
            if other.is_none() || other.as_deref() == Some(action) {
                self.flash(
                    format!("{name} already does that"),
                    "Mapping a key to itself would change nothing — choose a different output.",
                );
                return;
            }
        }
        self.undo = Some(Undo::Maps(self.profile_maps.clone()));
        let maps = self.profile_maps.entry(self.profile.clone()).or_default();
        let entry = if let Some(index) = maps.iter().position(|(key, _)| key == code) {
            &mut maps[index].1
        } else {
            maps.push((code.to_owned(), Mapping::default()));
            &mut maps.last_mut().unwrap().1
        };
        if self.mode == Mode::Hold {
            entry.hold = Some(action.to_owned());
        } else {
            entry.tap = Some(action.to_owned());
        }
        entry.device = self.device.clone();

        let held = if self.mode == Mode::Hold { " held" } else { "" };
        let device = self.device_label(&self.device.clone()).to_lowercase();
        self.flash(
            format!("{}{held} → {action}", key_name(code)),
            format!("Saved to xremap config · {device}"),
        );
        self.persist();
    }

    /// Snapshot, then mutate the active profile's shortcut groups.
    fn mutate_groups(&mut self, mutate: impl FnOnce(&mut Vec<Group>)) {
        if self.view == View::Tester {
            return;
        }
        self.undo = Some(Undo::Groups(self.profile_groups.clone()));
        mutate(self.profile_groups.entry(self.profile.clone()).or_default());
    }

    /// Add (or replace) a modifier combo on the selected key.
    fn add_combo(&mut self, action: &str) {
        let Some(selected) = self.selected else {
            return;
        };
        let from_mods: Vec<String> = model::MODS
            .iter()
            .zip(self.from_mods)
            .filter(|(_, on)| *on)
            .map(|(name, _)| (*name).to_owned())
            .collect();
        if from_mods.is_empty() {
            return;
        }
        let to_mods: Vec<String> = model::MODS
            .iter()
            .zip(self.to_mods)
            .filter(|(_, on)| *on)
            .map(|(name, _)| (*name).to_owned())
            .collect();
        let key = key_name(selected);
        let rule = Rule {
            from: Chord {
                mods: from_mods.clone(),
                key: key.clone(),
            },
            to: Chord {
                mods: to_mods.clone(),
                key: action.to_owned(),
            },
            note: String::new(),
        };
        self.mutate_groups(|groups| {
            let index = groups.iter().position(|group| group.id == "kb");
            let index = index.unwrap_or_else(|| {
                groups.insert(
                    0,
                    Group {
                        id: "kb".to_owned(),
                        name: "From the keyboard".to_owned(),
                        apps: Vec::new(),
                        enabled: true,
                        any_mod: false,
                        rules: Vec::new(),
                    },
                );
                0
            });
            let rules = &mut groups[index].rules;
            if let Some(same) = rules.iter().position(|other| {
                other.from.key == rule.from.key && other.from.mods == rule.from.mods
            }) {
                rules[same] = rule;
            } else {
                rules.push(rule);
            }
        });
        let output = if to_mods.is_empty() {
            action.to_owned()
        } else {
            format!("{}+{action}", to_mods.join("+"))
        };
        self.flash(
            format!("{}+{key} → {output}", from_mods.join("+")),
            "updated in preview · all applications",
        );
    }

    /// Restore the selected key to its default behavior.
    fn clear_mapping(&mut self) {
        if self.view == View::Tester {
            return;
        }
        let Some(code) = self.selected else {
            return;
        };
        self.remove_mapping(code);
    }

    /// Remove one key's mapping from the active profile.
    fn remove_mapping(&mut self, code: &str) {
        self.undo = Some(Undo::Maps(self.profile_maps.clone()));
        if let Some(maps) = self.profile_maps.get_mut(&self.profile) {
            maps.retain(|(key, _)| key != code);
        }
        self.flash(
            format!("{} back to default", key_name(code)),
            "Saved to xremap config",
        );
        self.persist();
    }

    /// Record a chord into the rule opened by the shortcut editor.
    fn set_chord(&mut self, side: Side, chord: Chord) {
        let Some(edit) = self.edit_rule else {
            return;
        };
        let new_index = self
            .groups()
            .get(edit.group)
            .map_or(0, |group| group.rules.len());
        self.mutate_groups(|groups| {
            let Some(group) = groups.get_mut(edit.group) else {
                return;
            };
            match edit.rule {
                Some(index) => {
                    if let Some(rule) = group.rules.get_mut(index) {
                        match side {
                            Side::From => rule.from = chord,
                            Side::To => rule.to = chord,
                        }
                    }
                }
                None => {
                    let mut rule = Rule::default();
                    match side {
                        Side::From => rule.from = chord,
                        Side::To => rule.to = chord,
                    }
                    group.rules.push(rule);
                }
            }
        });
        if edit.rule.is_none() {
            self.edit_rule = Some(EditRule {
                group: edit.group,
                rule: Some(new_index),
            });
        }
    }

    /// Create a new (optionally duplicated) profile and switch to it.
    fn create_profile(&mut self, duplicate: bool) {
        if self.view == View::Tester {
            return;
        }
        self.custom_profiles += 1;
        let id = format!("custom-{}", self.custom_profiles);
        let name = if duplicate {
            format!("{} copy", self.profile_name())
        } else {
            format!("Untitled {}", self.custom_profiles)
        };
        let maps = if duplicate {
            self.maps().clone()
        } else {
            Vec::new()
        };
        let groups = if duplicate {
            self.groups().to_vec()
        } else {
            Vec::new()
        };
        self.profiles.push(Profile {
            id: id.clone(),
            name: name.clone(),
        });
        self.profile_maps.insert(id.clone(), maps);
        self.profile_groups.insert(id.clone(), groups);
        self.profile = id;
        self.popover = None;
        self.selected = None;
        self.edit_rule = None;
        self.capture = false;
        self.recording = None;
        self.undo = None;
        let sub = if duplicate {
            "A separate copy of your mappings and shortcuts."
        } else {
            "Click a key to add your first mapping."
        };
        self.flash(format!("{name} created"), sub);
        self.persist();
    }

    /// Handle a physical key press reported by the evdev monitor.
    fn phys_press(&mut self, device: &PathBuf, scancode: u16) {
        use evdev::KeyCode as K;

        let escape = scancode == K::KEY_ESC.0;
        if escape && (self.capture || self.recording.is_some()) {
            self.capture = false;
            self.recording = None;
            return;
        }
        if escape {
            // Close the topmost surface first, like a native app.
            if self.popover.is_some() {
                self.popover = None;
            } else if self.remaps_open {
                self.remaps_open = false;
            } else if self.onboarding {
                self.onboarding = false;
                self.onb_step = 0;
            } else if self.selected.is_some() || self.edit_rule.is_some() {
                self.selected = None;
                self.edit_rule = None;
                self.recording = None;
            }
        }

        let is_modifier = [
            K::KEY_LEFTSHIFT.0,
            K::KEY_RIGHTSHIFT.0,
            K::KEY_LEFTCTRL.0,
            K::KEY_RIGHTCTRL.0,
            K::KEY_LEFTALT.0,
            K::KEY_RIGHTALT.0,
            K::KEY_LEFTMETA.0,
            K::KEY_RIGHTMETA.0,
        ]
        .contains(&scancode);

        // Recording a chord for the shortcut editor.
        if self.view == View::Shortcuts
            && let Some(side) = self.recording
            && self.edit_rule.is_some()
        {
            if is_modifier {
                return;
            }
            let Some(cap) = key_by_evdev(scancode) else {
                return;
            };
            let [ctrl, shift, alt, sup] = self.held_mods();
            let mods = [ctrl, shift, alt, sup]
                .iter()
                .zip(model::MODS)
                .filter(|(on, _)| **on)
                .map(|(_, name)| name.to_owned())
                .collect();
            self.set_chord(
                side,
                Chord {
                    mods,
                    key: key_name(cap.code),
                },
            );
            self.recording = None;
            self.pressed.insert((device.clone(), scancode));
            return;
        }

        // Recording an output key for the key editor.
        if self.view == View::Keyboard
            && self.capture
            && let Some(selected) = self.selected
        {
            let Some(cap) = key_by_evdev(scancode) else {
                return;
            };
            self.capture = false;
            self.pressed.insert((device.clone(), scancode));
            let action = key_name(cap.code);
            if self.mode == Mode::Combo {
                self.add_combo(&action);
            } else {
                self.set_mapping(selected, &action);
            }
            return;
        }

        self.pressed.insert((device.clone(), scancode));
        if let Some(cap) = key_by_evdev(scancode) {
            let name = self
                .devices
                .iter()
                .find(|entry| &entry.path == device)
                .map_or_else(|| "keyboard".to_owned(), |entry| entry.name.clone());
            self.last = Some(LastKey {
                code: cap.code,
                device: format!("Input received from {name}"),
            });
        }
        if self.onboarding && self.onb_step == 0 {
            self.onb_step = 1;
        }
    }
}

impl cosmic::Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = config::APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, (): Self::Flags) -> (Self, Task<Message>) {
        // Tests drive the update loop against the demo state; never
        // read or write the real ~/.config from them.
        let settings = if cfg!(test) {
            None
        } else {
            KeyloomConfig::handle()
        };
        let stored = settings
            .as_ref()
            .map(KeyloomConfig::load)
            .filter(|stored| !stored.profiles.is_empty());

        let (profiles, profile_maps, profile, custom_profiles) = match stored {
            Some(stored) => {
                let (profiles, maps, active, custom) = stored.into_state();
                (profiles, maps, active, custom as usize)
            }
            None => {
                // Fresh install: seed the demo profiles until the user
                // makes a change worth saving.
                let mut profiles = Vec::new();
                let mut profile_maps = HashMap::new();
                for (profile, maps) in model::demo_profiles() {
                    profile_maps.insert(profile.id.clone(), maps);
                    profiles.push(profile);
                }
                (profiles, profile_maps, "default".to_owned(), 0)
            }
        };
        let profile_groups: HashMap<String, Vec<Group>> =
            model::demo_groups().into_iter().collect();

        let mut app = App {
            core,
            view: View::Keyboard,
            popover: None,
            toast: None,
            toast_seq: 0,
            remaps_open: false,
            onboarding: false,
            onb_step: 0,
            profiles,
            profile,
            profile_maps,
            profile_groups,
            custom_profiles,
            undo: None,
            device: "all".to_owned(),
            form: keyboard::FORM_FULL,
            // Tests must not depend on the machine's xkb configuration.
            iso: !cfg!(test) && keyboard::detect_iso().unwrap_or(false),
            form_overridden: false,
            layer: Layer::Base,
            layers_open: false,
            selected: None,
            mode: Mode::Tap,
            query: String::new(),
            category: None,
            advanced: false,
            capture: false,
            from_mods: [true, false, false, false],
            to_mods: [false; 4],
            edit_rule: None,
            recording: None,
            sheet_opened: None,
            devices: Vec::new(),
            monitor_started: false,
            pressed: HashSet::new(),
            last: None,
            settings,
        };

        app.set_header_title(String::new());
        // Bring the generated file in line with the loaded state, but
        // never displace a hand-written config just for launching.
        if !cfg!(test) {
            app.write_xremap(false);
        }

        (app, Task::none())
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        ui::header::start(self)
    }

    fn header_center(&self) -> Vec<Element<'_, Message>> {
        ui::header::center(self)
    }

    fn header_end(&self) -> Vec<Element<'_, Message>> {
        ui::header::end(self)
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetView(view) => {
                self.view = view;
                self.popover = None;
                self.recording = None;
                self.remaps_open = false;
                match view {
                    View::Keyboard => self.edit_rule = None,
                    View::Tester => {
                        self.capture = false;
                        self.selected = None;
                        self.onboarding = false;
                        self.toast = None;
                        self.edit_rule = None;
                    }
                    View::Shortcuts => {
                        self.capture = false;
                        self.selected = None;
                    }
                }
            }
            Message::TogglePopover(popover) => {
                self.popover = if self.popover == Some(popover) {
                    None
                } else {
                    Some(popover)
                };
            }
            Message::CloseOverlays => self.popover = None,
            Message::SelectProfile(id) => {
                self.profile = id;
                self.capture = false;
                self.toast = None;
                self.undo = None;
                self.popover = None;
                self.remaps_open = false;
                self.selected = None;
                self.edit_rule = None;
                self.recording = None;
                let toast = self.flash(
                    format!("{} profile active", self.profile_name()),
                    "switched in place",
                );
                self.persist();
                // The profile-switch confirmation dismisses itself.
                return cosmic::task::future(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
                    Message::ToastExpired(toast)
                });
            }
            Message::NewProfile { duplicate } => self.create_profile(duplicate),
            Message::SelectDevice(id) => {
                self.device = id;
                self.popover = None;
            }
            Message::SetForm(form) => {
                self.form = form.min(keyboard::FORM_FACTORS.len() - 1);
                self.form_overridden = true;
                self.popover = None;
            }
            Message::SetVariant(iso) => {
                self.iso = iso;
                self.popover = None;
            }
            Message::OpenRemaps => {
                if self.view == View::Keyboard {
                    self.remaps_open = true;
                }
            }
            Message::CloseRemaps => self.remaps_open = false,
            Message::SetLayer(layer) => {
                self.layer = layer;
                if layer == Layer::Nav {
                    self.selected = None;
                }
            }
            Message::ToggleLayers => self.layers_open = !self.layers_open,
            Message::SelectKey(code) => {
                if self.view == View::Tester {
                    self.last = Some(LastKey {
                        code,
                        device: "Clicked in this preview".to_owned(),
                    });
                } else {
                    if self.selected != Some(code) {
                        self.toast = None;
                    }
                    if self.selected.is_none() {
                        // Opening (not switching keys) starts the rise.
                        self.sheet_opened = Some(Instant::now());
                    }
                    self.selected = Some(code);
                    self.advanced = false;
                    self.mode = Mode::Tap;
                    self.query.clear();
                    self.category = None;
                    self.capture = false;
                    self.popover = None;
                    self.remaps_open = false;
                }
            }
            Message::Query(query) => self.query = query,
            Message::SetCategory(category) => {
                self.category = Some(category);
                self.query.clear();
            }
            Message::PickAction(action) => {
                if self.mode == Mode::Combo {
                    self.add_combo(&action);
                } else if let Some(selected) = self.selected {
                    self.set_mapping(selected, &action);
                }
            }
            Message::ToggleAdvanced => {
                self.advanced = !self.advanced;
                self.mode = Mode::Tap;
                self.capture = false;
            }
            Message::SetMode(mode) => self.mode = mode,
            Message::ToggleFromMod(index) => {
                if let Some(state) = self.from_mods.get_mut(index) {
                    *state = !*state;
                }
            }
            Message::ToggleToMod(index) => {
                if let Some(state) = self.to_mods.get_mut(index) {
                    *state = !*state;
                }
            }
            Message::RemoveCombo { group, rule } => {
                self.mutate_groups(|groups| {
                    if let Some(group) = groups.get_mut(group)
                        && rule < group.rules.len()
                    {
                        group.rules.remove(rule);
                    }
                });
                self.flash("Combination removed", "applied instantly");
            }
            Message::ToggleSwap => {
                let Some(selected) = self.selected else {
                    return Task::none();
                };
                let Some(mapping) = self.mapping(selected) else {
                    return Task::none();
                };
                let Some(tap) = mapping.tap.clone() else {
                    return Task::none();
                };
                self.undo = Some(Undo::Maps(self.profile_maps.clone()));
                let mut swapped = false;
                if let Some(maps) = self.profile_maps.get_mut(&self.profile)
                    && let Some((_, entry)) = maps.iter_mut().find(|(key, _)| key == selected)
                {
                    entry.swap = !entry.swap;
                    swapped = entry.swap;
                }
                self.flash(
                    if swapped {
                        "Two-way swap on"
                    } else {
                        "Two-way swap off"
                    },
                    format!("{} ⇄ {tap}", key_name(selected)),
                );
                self.persist();
            }
            Message::ClearKey => self.clear_mapping(),
            Message::RemoveMapping(code) => {
                if self.view != View::Tester {
                    self.remove_mapping(&code);
                }
            }
            Message::ClosePanel => {
                self.selected = None;
                self.capture = false;
                self.toast = None;
            }
            Message::SetCapture(capture) => {
                if !capture || (self.view == View::Keyboard && self.selected.is_some()) {
                    self.capture = capture;
                }
            }
            Message::AddGroup => {
                let count = self.groups().len();
                self.mutate_groups(|groups| {
                    groups.push(Group {
                        id: format!("g{}", count + 1),
                        name: "New group".to_owned(),
                        apps: Vec::new(),
                        enabled: true,
                        any_mod: false,
                        rules: Vec::new(),
                    });
                });
                self.flash("Group added", "");
            }
            Message::ToggleGroup(index) => {
                let mut label = None;
                self.mutate_groups(|groups| {
                    if let Some(group) = groups.get_mut(index) {
                        group.enabled = !group.enabled;
                        label = Some((group.name.clone(), group.enabled));
                    }
                });
                if let Some((name, enabled)) = label {
                    self.flash(
                        format!("{name} {}", if enabled { "active" } else { "paused" }),
                        "applied instantly",
                    );
                }
            }
            Message::EditRule { group, rule } => {
                if self.edit_rule.is_none() {
                    self.sheet_opened = Some(Instant::now());
                }
                self.edit_rule = Some(EditRule { group, rule });
                self.selected = None;
                self.recording = if rule.is_none() {
                    Some(Side::From)
                } else {
                    None
                };
            }
            Message::SetRecording(side) => self.recording = side,
            Message::ToggleAnyMod => {
                let Some(edit) = self.edit_rule else {
                    return Task::none();
                };
                self.mutate_groups(|groups| {
                    if let Some(group) = groups.get_mut(edit.group) {
                        group.any_mod = !group.any_mod;
                    }
                });
            }
            Message::DeleteRule => {
                if let Some(edit) = self.edit_rule {
                    self.mutate_groups(|groups| {
                        if let Some(group) = groups.get_mut(edit.group)
                            && let Some(index) = edit.rule
                            && index < group.rules.len()
                        {
                            group.rules.remove(index);
                        }
                    });
                    self.flash("Shortcut removed", "applied instantly");
                    self.edit_rule = None;
                    self.recording = None;
                }
            }
            Message::CloseEdit => {
                self.edit_rule = None;
                self.recording = None;
            }
            Message::Undo => {
                if self.view == View::Tester {
                    return Task::none();
                }
                match self.undo.take() {
                    Some(Undo::Maps(maps)) => {
                        self.profile_maps = maps;
                        self.toast = None;
                        self.persist();
                    }
                    Some(Undo::Groups(groups)) => {
                        self.profile_groups = groups;
                        self.edit_rule = None;
                        self.recording = None;
                        self.toast = None;
                    }
                    None => self.toast = None,
                }
            }
            Message::ToastExpired(id) => {
                if self.toast.as_ref().is_some_and(|toast| toast.id == id) {
                    self.toast = None;
                }
            }
            // The redraw itself re-reads the animation clock.
            Message::SheetAnimate => {}
            Message::MenuShowSetup => {
                self.popover = None;
                self.onboarding = true;
                self.onb_step = 0;
                self.selected = None;
            }
            Message::MenuReset => {
                self.undo = Some(Undo::Maps(self.profile_maps.clone()));
                self.profile_maps.insert(self.profile.clone(), Vec::new());
                self.popover = None;
                self.selected = None;
                self.flash("Profile cleared", "nothing is remapped");
                self.persist();
            }
            Message::MenuAbout => {
                self.popover = None;
                self.undo = None;
                self.flash(
                    "Keyloom",
                    "Mappings are saved and written to your xremap config automatically.",
                );
            }
            Message::SkipOnboarding => {
                self.onboarding = false;
                self.onb_step = 0;
            }
            Message::NextOnboarding => {
                if self.view == View::Tester {
                    return Task::none();
                }
                if self.onb_step >= 2 {
                    self.onboarding = false;
                    self.onb_step = 0;
                    self.view = View::Keyboard;
                } else if self.onb_step == 1 {
                    self.undo = Some(Undo::Maps(self.profile_maps.clone()));
                    let maps = self.profile_maps.entry(self.profile.clone()).or_default();
                    if let Some(index) = maps.iter().position(|(key, _)| key == "CapsLock") {
                        maps[index].1.tap = Some("Escape".to_owned());
                        maps[index].1.device = "all".to_owned();
                    } else {
                        maps.push((
                            "CapsLock".to_owned(),
                            Mapping {
                                tap: Some("Escape".to_owned()),
                                device: "all".to_owned(),
                                ..Mapping::default()
                            },
                        ));
                    }
                    self.onb_step = 2;
                    self.persist();
                } else {
                    self.onb_step += 1;
                }
            }
            Message::Monitor(event) => match event {
                monitor::Event::Started(devices) => {
                    self.devices = devices;
                    self.monitor_started = true;
                    self.pressed.clear();
                    // Default the deck to the detected size until the
                    // user picks one; the picker always wins.
                    if !self.form_overridden
                        && let Some(form) = monitor::detected_form(self.devices.iter())
                    {
                        self.form = form;
                    }
                }
                monitor::Event::Key { device, event } => match event {
                    monitor::KeyEvent::Pressed(code) => self.phys_press(&device, code),
                    monitor::KeyEvent::Repeated(_) => {}
                    monitor::KeyEvent::Released(code) => {
                        self.pressed.remove(&(device, code));
                    }
                },
                monitor::Event::Disconnected(path) => {
                    if let Some(device) = self.devices.iter_mut().find(|device| device.path == path)
                    {
                        device.connected = false;
                    }
                    self.pressed.retain(|(device, _)| *device != path);
                }
            },
        }

        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![Subscription::run(monitor_stream)];
        // Drive redraws only while the sheet is actively rising.
        if self.sheet_open() && self.sheet_progress() < 1.0 {
            subscriptions.push(cosmic::iced::window::frames().map(|_| Message::SheetAnimate));
        }
        Subscription::batch(subscriptions)
    }

    /// Modal dialogs render natively above the window content.
    fn dialog(&self) -> Option<Element<'_, Message>> {
        if self.view == View::Keyboard && self.selected.is_some() && self.capture {
            return Some(ui::overlays::capture_dialog(self));
        }
        if self.view == View::Keyboard && self.remaps_open {
            return Some(ui::overlays::remaps_dialog(self));
        }
        if self.onboarding && self.view != View::Tester {
            return Some(ui::overlays::onboarding(self));
        }
        None
    }

    fn view(&self) -> Element<'_, Message> {
        ui::view(self)
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

    fn app() -> App {
        App::init(Core::default(), ()).0
    }

    #[test]
    fn selecting_action_maps_selected_key() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));

        assert_eq!(
            app.mapping("CapsLock").and_then(|m| m.tap.as_deref()),
            Some("Escape")
        );
        let toast = app.toast.as_ref().expect("mapping shows a toast");
        assert_eq!(toast.text, "Caps Lock → Escape");
    }

    #[test]
    fn undo_restores_previous_mappings() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("KeyA"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        assert!(app.mapping("KeyA").is_some());

        let _ = app.update(Message::Undo);
        assert!(app.mapping("KeyA").is_none());
    }

    #[test]
    fn profile_switch_toast_expires_by_id() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("laptop".to_owned()));
        let id = app.toast.as_ref().expect("switch shows a toast").id;

        // A stale expiry (e.g. from an earlier toast) is ignored.
        let _ = app.update(Message::ToastExpired(id + 1));
        assert!(app.toast.is_some());

        let _ = app.update(Message::ToastExpired(id));
        assert!(app.toast.is_none(), "toast auto-hides after its delay");
    }

    #[test]
    fn expiry_never_removes_a_newer_toast() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("laptop".to_owned()));
        let stale = app.toast.as_ref().unwrap().id;

        // A new mapping replaces the toast before the timer fires.
        let _ = app.update(Message::SelectKey("KeyA"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::ToastExpired(stale));

        let toast = app.toast.as_ref().expect("newer toast survives");
        assert_eq!(toast.text, "A → Escape");
    }

    #[test]
    fn selection_drives_the_bottom_sheet() {
        let mut app = app();
        assert!(app.selected.is_none());

        let _ = app.update(Message::SelectKey("CapsLock"));
        assert_eq!(app.selected, Some("CapsLock"), "selecting opens the sheet");

        let _ = app.update(Message::ClosePanel);
        assert!(app.selected.is_none(), "done closes the sheet");

        let _ = app.update(Message::SetView(View::Shortcuts));
        let _ = app.update(Message::EditRule {
            group: 0,
            rule: None,
        });
        assert!(app.edit_rule.is_some(), "editing opens the sheet");

        let _ = app.update(Message::SetView(View::Keyboard));
        assert!(app.edit_rule.is_none(), "switching views closes the sheet");
    }

    #[test]
    fn opening_the_sheet_starts_the_rise_animation() {
        let mut app = app();
        assert!(
            (app.sheet_progress() - 1.0).abs() < f32::EPSILON,
            "closed sheet reports settled progress"
        );

        let _ = app.update(Message::SelectKey("CapsLock"));
        let started = app.sheet_opened.expect("opening starts the rise");
        assert!(app.sheet_progress() < 1.0);

        // Switching keys while open must not replay the animation.
        let _ = app.update(Message::SelectKey("KeyA"));
        assert_eq!(app.sheet_opened, Some(started));

        // Reopening after closing rises again.
        let _ = app.update(Message::ClosePanel);
        let _ = app.update(Message::SelectKey("KeyB"));
        assert!(app.sheet_opened.expect("reopened") >= started);
    }

    #[test]
    fn remaps_dialog_opens_and_selecting_a_row_closes_it() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("laptop".to_owned()));
        let _ = app.update(Message::OpenRemaps);
        assert!(app.remaps_open);

        let _ = app.update(Message::SelectKey("CapsLock"));
        assert!(!app.remaps_open, "picking a remap closes the dialog");
        assert_eq!(app.selected, Some("CapsLock"));
    }

    #[test]
    fn remaps_dialog_only_opens_in_keyboard_view() {
        let mut app = app();
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::OpenRemaps);
        assert!(!app.remaps_open);

        let _ = app.update(Message::SetView(View::Keyboard));
        let _ = app.update(Message::OpenRemaps);
        assert!(app.remaps_open);
        let _ = app.update(Message::SetView(View::Shortcuts));
        assert!(!app.remaps_open, "leaving the view closes the dialog");
    }

    #[test]
    fn hold_mode_sets_hold_action() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::SetMode(Mode::Hold));
        let _ = app.update(Message::PickAction("Control".to_owned()));

        let mapping = app.mapping("CapsLock").expect("mapping created");
        assert_eq!(mapping.hold.as_deref(), Some("Control"));
        assert_eq!(mapping.tap, None);
    }

    #[test]
    fn mapping_a_key_to_itself_is_rejected() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Caps Lock".to_owned()));

        assert!(app.mapping("CapsLock").is_none(), "self-map not stored");
        let toast = app.toast.as_ref().expect("rejection is explained");
        assert_eq!(toast.text, "Caps Lock already does that");

        // A hold-only self-map is equally pointless.
        let _ = app.update(Message::SetMode(Mode::Hold));
        let _ = app.update(Message::PickAction("Caps Lock".to_owned()));
        assert!(app.mapping("CapsLock").is_none());
    }

    #[test]
    fn self_tap_is_allowed_when_hold_differs() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::SetMode(Mode::Hold));
        let _ = app.update(Message::PickAction("Left Control".to_owned()));

        // Keep the tap as Caps Lock itself while hold becomes Control.
        let _ = app.update(Message::SetMode(Mode::Tap));
        let _ = app.update(Message::PickAction("Caps Lock".to_owned()));

        let mapping = app.mapping("CapsLock").expect("mapping kept");
        assert_eq!(mapping.tap.as_deref(), Some("Caps Lock"));
        assert_eq!(mapping.hold.as_deref(), Some("Left Control"));
    }

    #[test]
    fn remove_mapping_from_the_remaps_list() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("laptop".to_owned()));
        let count = app.maps().len();
        assert!(app.mapping("CapsLock").is_some());

        let _ = app.update(Message::RemoveMapping("CapsLock".to_owned()));
        assert!(app.mapping("CapsLock").is_none());
        assert_eq!(app.maps().len(), count - 1);

        let _ = app.update(Message::Undo);
        assert!(app.mapping("CapsLock").is_some(), "removal is undoable");
    }

    #[test]
    fn switching_decks_preserves_mappings_and_yaml() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("Numpad7"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let yaml = crate::xremap::generate(app.maps(), |id| id.to_owned());

        // Numpad7 is absent from the 60% deck; the rule must survive
        // switching there and back, and the generated YAML must not
        // change.
        let _ = app.update(Message::SetForm(keyboard::FORM_SIXTY));
        let _ = app.update(Message::SetVariant(true));
        assert_eq!(
            app.mapping("Numpad7").and_then(|m| m.tap.as_deref()),
            Some("Escape")
        );
        assert_eq!(
            crate::xremap::generate(app.maps(), |id| id.to_owned()),
            yaml
        );

        let _ = app.update(Message::SetForm(keyboard::FORM_FULL));
        assert_eq!(
            app.mapping("Numpad7").and_then(|m| m.tap.as_deref()),
            Some("Escape")
        );
    }

    fn started(form: usize, form_hinted: bool) -> Message {
        Message::Monitor(monitor::Event::Started(vec![monitor::KeyboardDevice {
            path: PathBuf::from("/dev/input/event0"),
            name: "Test Keyboard".to_owned(),
            connected: true,
            form,
            form_hinted,
        }]))
    }

    #[test]
    fn detected_form_defaults_the_deck() {
        let mut app = app();
        assert_eq!(app.form, keyboard::FORM_FULL);

        let _ = app.update(started(keyboard::FORM_TKL, true));
        assert_eq!(app.form, keyboard::FORM_TKL, "detection picks the deck");
    }

    #[test]
    fn manual_size_choice_beats_detection() {
        let mut app = app();
        let _ = app.update(Message::SetForm(keyboard::FORM_SIXTY_FIVE));
        assert_eq!(app.form, keyboard::FORM_SIXTY_FIVE);

        let _ = app.update(started(keyboard::FORM_TKL, true));
        assert_eq!(
            app.form,
            keyboard::FORM_SIXTY_FIVE,
            "the size picker always wins over detection"
        );
    }

    #[test]
    fn duplicate_profile_copies_mappings() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("laptop".to_owned()));
        let count = app.maps().len();
        assert!(count > 0);

        let _ = app.update(Message::NewProfile { duplicate: true });
        assert_eq!(app.profile_name(), "Laptop copy");
        assert_eq!(app.maps().len(), count);
    }

    #[test]
    fn onboarding_demo_applies_caps_to_escape() {
        let mut app = app();
        let _ = app.update(Message::MenuShowSetup);
        assert!(app.onboarding);

        let _ = app.update(Message::NextOnboarding); // step 0 -> 1
        let _ = app.update(Message::NextOnboarding); // applies Caps -> Esc
        assert_eq!(
            app.mapping("CapsLock").and_then(|m| m.tap.as_deref()),
            Some("Escape")
        );

        let _ = app.update(Message::NextOnboarding); // closes
        assert!(!app.onboarding);
    }

    #[test]
    fn tester_click_reports_key_without_mapping() {
        let mut app = app();
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::SelectKey("KeyQ"));

        assert!(app.selected.is_none());
        assert_eq!(app.last.as_ref().map(|last| last.code), Some("KeyQ"));
    }

    #[test]
    fn combo_mode_adds_shortcut_rule() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("KeyC"));
        let _ = app.update(Message::SetMode(Mode::Combo));
        let _ = app.update(Message::PickAction("Copy".to_owned()));

        let combos = app.combos_for("C");
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].2.from.mods, vec!["Ctrl".to_owned()]);
        assert_eq!(combos[0].2.to.key, "Copy");
    }
}

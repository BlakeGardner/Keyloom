//! Application state and update logic for the Keyloom GUI.
//!
//! The interface follows the design export in `design/`. Profiles and
//! their mappings persist via cosmic-config, and every change
//! regenerates the xremap configuration written to the user's config
//! directory and restarts the xremap user service (debounced) so it
//! takes effect; shortcut groups are still previewed in memory only.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, ConfigSet, CosmicConfigEntry};
use cosmic::iced::Subscription;
use cosmic::iced::futures::{Stream, StreamExt};
use cosmic::prelude::*;

use crate::config::{self, KeyboardLayouts, KeyloomConfig, LayoutOverride, SetupState};
use crate::keyboard;
use crate::monitor;
use crate::service;
use crate::setup;
use crate::ui;
use crate::ui::model::{self, Chord, Group, Mapping, Maps, Profile, Rule, key_by_evdev, key_name};
use crate::xremap;

/// How long the bottom sheet takes to open or close (the design's `kbRise`).
const SHEET_ANIMATION: Duration = Duration::from_millis(220);

/// Quiet period between the last config change and the automatic
/// service restart that applies it.
const APPLY_DEBOUNCE: Duration = Duration::from_millis(600);

/// Minimum spacing between restarts. Rapid-fire restarts would trip
/// systemd's default start rate limit (5 starts per 10 s) and leave
/// the unit failed, so applies are paced well under it.
const APPLY_MIN_GAP: Duration = Duration::from_millis(2500);

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
    /// A deleted profile: its list position, mappings, and groups.
    Profile {
        index: usize,
        profile: Profile,
        maps: Maps,
        groups: Vec<Group>,
    },
}

/// What a due apply should do, given the current state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApplyStep {
    /// Superseded by a newer change whose own apply is still coming.
    Stale,
    /// Nothing to restart: there is no unit, or remapping is not
    /// running and will pick the file up when it is started.
    Skip,
    /// Busy or too soon after the last restart; retry after the delay.
    Wait(Duration),
    /// Restart the service now.
    Restart,
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

/// The page first-run setup is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupPage {
    Welcome,
    Step(setup::Step),
    Finish,
}

/// First-run setup while it is open: a wizard over the system checks
/// in [`crate::setup`].
#[derive(Clone, Debug)]
pub struct Setup {
    pub page: SetupPage,
    /// What the last probe found; `None` until the first one lands.
    pub facts: Option<setup::Facts>,
    /// A probe is running.
    pub probing: bool,
    /// The step whose fix is running.
    pub busy: Option<setup::Step>,
    /// Why the last fix failed, and which step it belonged to.
    pub error: Option<(setup::Step, setup::ActionError)>,
}

impl Setup {
    fn new() -> Self {
        Self {
            page: SetupPage::Welcome,
            facts: None,
            probing: true,
            busy: None,
            error: None,
        }
    }
}

/// Reverse animation retained while a dismissed sheet slides out.
#[derive(Clone, Copy, Debug)]
struct SheetCloseAnimation {
    started: Instant,
    from_progress: f32,
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
    /// Ask for confirmation before deleting this profile (never the
    /// active one).
    DeleteProfile(String),
    /// Delete the profile pending confirmation.
    DeleteConfirm,
    /// Dismiss the delete confirmation dialog.
    DeleteCancel,
    /// Start (or cancel) renaming the active profile.
    RenameToggle,
    RenameInput(String),
    RenameCommit,
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
    /// Ask for confirmation before removing a mapping from the remaps list.
    RemoveMapping(String),
    RemoveMappingConfirm,
    RemoveMappingCancel,
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
    /// A debounced apply came due; the id lets a newer change
    /// supersede it. Applying restarts the xremap service so it
    /// re-reads the written config.
    Apply(u64),
    /// Result of the restart an apply performed.
    Applied(Result<(), service::Error>),
    ServiceStatus(service::Status),
    /// Put remapping in this state, from the status chip.
    SetRemapping(service::Remapping),
    /// Result of the switch behind [`Message::SetRemapping`].
    RemappingSwitched {
        target: service::Remapping,
        result: Result<(), service::Error>,
    },
    /// Redraw tick while the bottom sheet opens or closes.
    SheetAnimate,
    /// Open first-run setup: from the menu, from the header chip when
    /// nothing is set up, and on its own the first time Keyloom runs.
    MenuShowSetup,
    /// What the setup checks found.
    SetupProbed(setup::Facts),
    /// Show another page of setup.
    SetupPage(SetupPage),
    /// Run the setup checks again.
    SetupRecheck,
    /// Carry out the fix a setup step offers.
    SetupAct(setup::Step),
    /// The fix finished.
    SetupActed {
        step: setup::Step,
        result: Result<(), setup::ActionError>,
    },
    /// Close setup before its last page.
    SetupSkip,
    /// Close setup from its last page.
    SetupFinish,
    /// Apply the example remap and close setup.
    SetupExample,
    MenuReset,
    ResetMappingsConfirm,
    ResetMappingsCancel,
    MenuAbout,
    CloseAbout,
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
    /// Key code awaiting removal confirmation in the active profile.
    pub confirm_remove_mapping: Option<String>,
    pub about_open: bool,
    /// First-run setup, while it is open.
    pub setup: Option<Setup>,
    /// How far setup got, as remembered between launches.
    setup_state: SetupState,
    // Profiles and their in-memory preview state.
    pub profiles: Vec<Profile>,
    pub profile: String,
    pub profile_maps: HashMap<String, Maps>,
    pub profile_groups: HashMap<String, Vec<Group>>,
    custom_profiles: usize,
    /// In-progress rename of the active profile (the edited text).
    pub rename: Option<String>,
    /// Profile id awaiting delete confirmation in the modal dialog.
    pub confirm_delete: Option<String>,
    /// Active profile id awaiting confirmation before all its mappings are reset.
    pub confirm_reset_mappings: Option<String>,
    pub undo: Option<Undo>,
    // Keyboard view state.
    pub device: String,
    /// Displayed form factor (index into [`keyboard::FORM_FACTORS`]).
    pub form: usize,
    /// Whether the deck uses the ISO assembly instead of ANSI.
    pub iso: bool,
    keyboard_layouts: KeyboardLayouts,
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
    /// When the bottom sheet last started opening.
    sheet_opened: Option<Instant>,
    /// Closing animation, retained until its final frame clears the editor.
    sheet_closing: Option<SheetCloseAnimation>,
    // Hardware monitoring.
    pub devices: Vec<monitor::KeyboardDevice>,
    pub monitor_started: bool,
    pub pressed: HashSet<(PathBuf, u16)>,
    pub last: Option<LastKey>,
    /// Last known state of the xremap service (None until queried).
    pub service: Option<service::Status>,
    /// Input nodes the remapper holds exclusively (see
    /// [`monitor::Event::Grabbed`]). Those keyboards report nothing
    /// here until it lets them go.
    pub grabbed: HashSet<PathBuf>,
    /// The state a switch still on its way to the service is heading
    /// for. Whether remapping runs is [`Self::service`] alone — a unit
    /// Keyloom stopped is not a different state from one that was
    /// already stopped.
    pub switching: Option<service::Remapping>,
    /// Generation of the latest scheduled apply; a due apply carrying
    /// an older id has been superseded and is dropped.
    apply_seq: u64,
    /// An apply was scheduled during this update pass and its
    /// debounce task still has to be spawned.
    apply_pending: bool,
    /// A change is on its way to the running service — from being
    /// scheduled until its restart settles (or is skipped). Drives
    /// the header's "Applying Remaps" state.
    apply_outstanding: bool,
    /// The generation a restart in flight covers (None when idle).
    applying: Option<u64>,
    /// When the last restart was issued, for pacing.
    last_apply: Option<Instant>,
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
        self.devices
            .iter()
            .find(|device| device.path.to_string_lossy() == id)
            .map_or_else(|| id.to_owned(), |device| device.name.clone())
    }

    /// The device rows offered by the "Applies to" popover:
    /// `(id, name, sub)`.
    pub fn device_entries(&self) -> Vec<(String, String, String)> {
        let mut entries = vec![(
            "all".to_owned(),
            "All keyboards".to_owned(),
            if self.devices.is_empty() {
                "No keyboards detected".to_owned()
            } else {
                format!(
                    "{} detected keyboard{}",
                    self.devices.len(),
                    if self.devices.len() == 1 { "" } else { "s" }
                )
            },
        )];
        for device in &self.devices {
            let path = device.path.display();
            let sub = if !device.connected {
                format!("{path} (disconnected)")
            } else if monitor::is_remapper_output(device) {
                format!("{path} · remapped output")
            } else {
                path.to_string()
            };
            entries.push((
                device.path.to_string_lossy().into_owned(),
                device.name.clone(),
                sub,
            ));
        }
        entries
    }

    /// Whether a device contributes to the current view's live input.
    fn shows_input_from(&self, device: &Path) -> bool {
        self.view != View::Tester || self.device == "all" || device == Path::new(&self.device)
    }

    /// Whether a key is held, respecting the tester's device filter.
    pub fn is_pressed(&self, evdev: u16) -> bool {
        self.pressed
            .iter()
            .any(|(device, code)| *code == evdev && self.shows_input_from(device))
    }

    /// Live key-cap highlights are shown only in the tester.
    pub fn highlights_key(&self, evdev: u16) -> bool {
        self.view == View::Tester && self.is_pressed(evdev)
    }

    /// The key caps of the currently displayed deck.
    pub fn deck(&self) -> &'static [model::KeyCap] {
        model::deck(self.form, self.iso)
    }

    fn selected_device(&self) -> Option<&monitor::KeyboardDevice> {
        self.devices
            .iter()
            .find(|device| device.path == Path::new(&self.device))
    }

    /// The selected keyboard when the remapper is holding it, which is
    /// why the tester sees nothing from it.
    pub fn grabbed_selection(&self) -> Option<&monitor::KeyboardDevice> {
        self.selected_device()
            .filter(|device| self.grabbed.contains(&device.path))
    }

    /// The state pressing the status chip would leave remapping in.
    /// `None` leaves the chip passive — there is no unit to control, or
    /// a switch or restart is already on its way and would fight the
    /// request.
    pub fn remapping_toggle(&self) -> Option<service::Remapping> {
        if self.switching.is_some() || self.applying.is_some() {
            return None;
        }
        match self.service? {
            service::Status::Active => Some(service::Remapping::Off),
            service::Status::Inactive | service::Status::Failed => Some(service::Remapping::On),
            service::Status::NotFound | service::Status::Unavailable => None,
        }
    }

    /// Open first-run setup and start checking the system.
    fn open_setup(&mut self) -> Task<Message> {
        self.popover = None;
        self.rename = None;
        self.remaps_open = false;
        self.confirm_remove_mapping = None;
        self.clear_sheet();
        self.setup = Some(Setup::new());
        setup_probe_task()
    }

    /// Close setup, remembering whether it needs to come back on its
    /// own. Setup that found everything in order (or only waiting for
    /// a new login) is complete however it is closed; a completed
    /// setup never becomes incomplete by being reopened and closed.
    fn leave_setup(&mut self) {
        let Some(setup) = self.setup.take() else {
            return;
        };
        if setup
            .facts
            .as_ref()
            .is_some_and(setup::Facts::is_configured)
        {
            self.set_setup_state(SetupState::Complete);
        } else if self.setup_state != SetupState::Complete {
            self.set_setup_state(SetupState::Deferred);
        }
    }

    /// Record how far setup got, without touching the remaps.
    fn set_setup_state(&mut self, state: SetupState) {
        if self.setup_state == state {
            return;
        }
        self.setup_state = state;
        if let Some(settings) = &self.settings
            && let Err(err) = settings.set("setup", state)
        {
            eprintln!("keyloom: failed to save setup state: {err}");
        }
    }

    /// Run the fix a setup step offers; [`Message::SetupActed`]
    /// reports how it went. Steps without a fix, and steps whose fix
    /// is already running, do nothing.
    fn setup_act(&mut self, step: setup::Step) -> Task<Message> {
        let Some(setup) = &mut self.setup else {
            return Task::none();
        };
        if setup.busy.is_some() {
            return Task::none();
        }
        let Some(facts) = setup.facts.clone() else {
            return Task::none();
        };
        let task = match step {
            setup::Step::Xremap => return Task::none(),
            setup::Step::InputGroup => {
                let Some(user) = facts.user else {
                    return Task::none();
                };
                cosmic::task::future(async move {
                    Message::SetupActed {
                        step,
                        result: setup::add_to_input_group(&user).await,
                    }
                })
            }
            setup::Step::Uinput => cosmic::task::future(async move {
                Message::SetupActed {
                    step,
                    result: setup::prepare_uinput().await,
                }
            }),
            setup::Step::Service => {
                let Some(action) = facts.service_action() else {
                    return Task::none();
                };
                cosmic::task::future(async move {
                    Message::SetupActed {
                        step,
                        result: setup::run_service_action(action, &facts).await,
                    }
                })
            }
        };
        setup.busy = Some(step);
        setup.error = None;
        task
    }

    /// Setup's one-click example: Caps Lock → Escape in the active
    /// profile, shown on the keyboard and undoable like any change.
    fn apply_example(&mut self) {
        self.view = View::Keyboard;
        self.clear_sheet();
        self.undo = Some(Undo::Maps(self.profile_maps.clone()));
        let maps = self.profile_maps.entry(self.profile.clone()).or_default();
        match maps.iter_mut().find(|(key, _)| key == "CapsLock") {
            Some((_, mapping)) => {
                mapping.tap = Some("Escape".to_owned());
                mapping.device = "all".to_owned();
            }
            None => maps.push((
                "CapsLock".to_owned(),
                Mapping {
                    tap: Some("Escape".to_owned()),
                    device: "all".to_owned(),
                    ..Mapping::default()
                },
            )),
        }
        self.flash(
            "Caps Lock → Escape",
            "applies automatically · all keyboards",
        );
        self.persist();
    }

    /// Put remapping in the requested state. The service call runs in
    /// the background; [`Message::RemappingSwitched`] reports what
    /// happened.
    ///
    /// The request is never filtered against the state Keyloom last
    /// saw: a unit can be stopped without this session having stopped
    /// it (a previous run, or a `systemctl` in a terminal), and
    /// pressing the chip has to start it just the same.
    fn set_remapping(&mut self, target: service::Remapping) -> Task<Message> {
        self.switching = Some(target);
        cosmic::task::future(async move {
            Message::RemappingSwitched {
                target,
                result: service::set(target).await,
            }
        })
    }

    /// The selected keyboard's guess, or the aggregate for All keyboards.
    /// A disconnected selection keeps its last known guess.
    pub fn detected_form(&self) -> Option<usize> {
        if self.device == "all" {
            monitor::detected_form(self.devices.iter())
        } else {
            self.selected_device().map(|device| device.form)
        }
    }

    /// The selected keyboard's physical variant, or the aggregate for All.
    pub fn detected_iso(&self) -> Option<bool> {
        if self.device == "all" {
            monitor::detected_iso(self.devices.iter())
        } else {
            self.selected_device().map(|device| device.iso)
        }
    }

    /// Manual choices for the active scope; automatic axes remain absent.
    fn layout_override(&self) -> LayoutOverride {
        let mut choice = if self.device == "all" {
            self.keyboard_layouts.all
        } else {
            self.selected_device()
                .and_then(|device| self.keyboard_layouts.devices.get(&device.id))
                .copied()
                .unwrap_or_default()
        };
        choice.form = choice
            .form
            .filter(|form| *form < keyboard::FORM_FACTORS.len());
        choice
    }

    fn refresh_layout(&mut self) {
        let choice = self.layout_override();
        self.form = choice
            .form
            .or_else(|| self.detected_form())
            .unwrap_or(keyboard::FORM_FULL);
        self.iso = choice.iso.or_else(|| self.detected_iso()).unwrap_or(false);
    }

    /// Save display settings without rewriting remaps or restarting xremap.
    fn set_layout_override(&mut self, choice: LayoutOverride) {
        if self.device == "all" {
            self.keyboard_layouts.all = choice;
        } else if let Some(device) = self.selected_device() {
            let id = device.id.clone();
            if choice == LayoutOverride::default() {
                self.keyboard_layouts.devices.remove(&id);
            } else {
                self.keyboard_layouts.devices.insert(id, choice);
            }
        }
        self.refresh_layout();
        self.popover = None;
        // Tests may supply an isolated store; init never opens real settings
        // in a test, and display preferences have no external side effects.
        if let Some(settings) = &self.settings
            && let Err(err) = settings.set("keyboard_layouts", &self.keyboard_layouts)
        {
            eprintln!("keyloom: failed to save keyboard layouts: {err}");
            self.flash("Could not save keyboard layout", err.to_string());
        }
    }

    /// Held modifiers as `[Ctrl, Shift, Alt, Super]`.
    pub fn held_mods(&self) -> [bool; 4] {
        use evdev::KeyCode as K;
        let held = |codes: &[u16]| codes.iter().any(|code| self.is_pressed(*code));
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

    /// Whether the bottom editor sheet is open for interaction.
    pub fn sheet_open(&self) -> bool {
        self.sheet_visible() && self.sheet_closing.is_none()
    }

    /// Whether the bottom editor sheet still has content to render.
    pub fn sheet_visible(&self) -> bool {
        (self.view == View::Keyboard && self.selected.is_some())
            || (self.view == View::Shortcuts && self.edit_rule.is_some())
    }

    /// Linear visibility of the sheet from hidden (0.0) to open (1.0).
    pub fn sheet_progress(&self) -> f32 {
        if !self.sheet_visible() {
            return 0.0;
        }
        if let Some(closing) = self.sheet_closing {
            return (closing.from_progress
                - closing.started.elapsed().as_secs_f32() / SHEET_ANIMATION.as_secs_f32())
            .max(0.0);
        }
        self.sheet_opened.map_or(1.0, |opened| {
            (opened.elapsed().as_secs_f32() / SHEET_ANIMATION.as_secs_f32()).min(1.0)
        })
    }

    /// Start opening from the sheet's current position, including if its
    /// closing animation is reversed by another selection.
    fn open_sheet(&mut self) {
        let progress = self.sheet_progress();
        self.sheet_closing = None;
        self.sheet_opened = Instant::now().checked_sub(SHEET_ANIMATION.mul_f32(progress));
    }

    /// Keep the editor state alive while the sheet animates out.
    fn close_sheet(&mut self) {
        if self.sheet_visible() && self.sheet_closing.is_none() {
            self.sheet_closing = Some(SheetCloseAnimation {
                started: Instant::now(),
                from_progress: self.sheet_progress(),
            });
        }
    }

    /// Clear a sheet after its closing animation, or immediately when a
    /// larger navigation transition makes retaining it inappropriate.
    fn clear_sheet(&mut self) {
        self.selected = None;
        self.edit_rule = None;
        self.capture = false;
        self.recording = None;
        self.sheet_opened = None;
        self.sheet_closing = None;
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
    /// Called after every change to profiles or their mappings; a
    /// changed file schedules the debounced apply.
    fn persist(&mut self) {
        // Tests exercise the update loop; never touch the real
        // ~/.config from them. The apply is still scheduled so the
        // debounce logic stays observable (its tasks never run).
        if cfg!(test) {
            self.schedule_apply();
            return;
        }
        if let Some(settings) = &self.settings {
            let snapshot = KeyloomConfig::snapshot(
                &self.profiles,
                &self.profile_maps,
                &self.profile,
                u32::try_from(self.custom_profiles).unwrap_or(u32::MAX),
                &self.keyboard_layouts,
                self.setup_state,
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
            // Only a real content change warrants a service restart.
            Ok(xremap::WriteOutcome::Written(_)) => self.schedule_apply(),
            Ok(xremap::WriteOutcome::Unchanged(_)) => {}
            Ok(xremap::WriteOutcome::SkippedForeign(path)) => {
                eprintln!(
                    "keyloom: leaving existing xremap config untouched: {}",
                    path.display()
                );
            }
            Err(err) => {
                self.flash("Could not save remaps", err.to_string());
            }
        }
    }

    /// Note that the on-disk config changed: the service should be
    /// restarted once changes stop for a moment. The debounce task is
    /// spawned by [`Self::pending_apply`] as the update pass ends.
    fn schedule_apply(&mut self) {
        self.apply_seq += 1;
        self.apply_pending = true;
        self.apply_outstanding = true;
    }

    /// Whether a change is still on its way to the running service
    /// (debounce pending or restart in flight).
    pub fn apply_in_progress(&self) -> bool {
        self.apply_outstanding || self.applying.is_some()
    }

    /// The debounce task for a scheduled apply, if one is due.
    fn pending_apply(&mut self) -> Task<Message> {
        if !self.apply_pending {
            return Task::none();
        }
        self.apply_pending = false;
        let seq = self.apply_seq;
        cosmic::task::future(async move {
            tokio::time::sleep(APPLY_DEBOUNCE).await;
            Message::Apply(seq)
        })
    }

    /// Decide what a due [`Message::Apply`] should do right now.
    fn apply_step(&self, seq: u64) -> ApplyStep {
        // A newer change owns the apply.
        if seq != self.apply_seq {
            return ApplyStep::Stale;
        }
        // Only a running service has anything to re-read, and only the
        // status chip starts a stopped one. A service started later
        // reads the file as it stands, so nothing is lost by skipping
        // the restart.
        if self.switching.is_some()
            || self
                .service
                .is_some_and(|status| status != service::Status::Active)
        {
            return ApplyStep::Skip;
        }
        if self.applying.is_some() {
            return ApplyStep::Wait(APPLY_DEBOUNCE);
        }
        match self.last_apply.map(|last| last.elapsed()) {
            Some(elapsed) if elapsed < APPLY_MIN_GAP => ApplyStep::Wait(APPLY_MIN_GAP - elapsed),
            _ => ApplyStep::Restart,
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
            format!("applies automatically · {device}"),
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
            "applies automatically",
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
        self.confirm_remove_mapping = None;
        self.confirm_reset_mappings = None;
        self.popover = None;
        self.rename = None;
        self.clear_sheet();
        self.undo = None;
        let sub = if duplicate {
            "A separate copy of your mappings and shortcuts."
        } else {
            "Click a key to add your first mapping."
        };
        self.flash(format!("{name} created"), sub);
        self.persist();
    }

    /// Delete a profile by id, once confirmed. The active profile is
    /// protected — which also guarantees at least one profile always
    /// remains — and the toast's Undo restores the deleted one in place.
    fn delete_profile(&mut self, id: &str) {
        if self.view == View::Tester || id == self.profile || self.profiles.len() <= 1 {
            return;
        }
        let Some(index) = self.profiles.iter().position(|profile| profile.id == id) else {
            return;
        };
        let profile = self.profiles.remove(index);
        let maps = self.profile_maps.remove(id).unwrap_or_default();
        let groups = self.profile_groups.remove(id).unwrap_or_default();
        let name = profile.name.clone();
        self.undo = Some(Undo::Profile {
            index,
            profile,
            maps,
            groups,
        });
        self.popover = None;
        self.flash(format!("{name} deleted"), "Undo restores it.");
        self.persist();
    }

    /// Apply the pending rename to the active profile.
    fn commit_rename(&mut self) {
        let Some(name) = self.rename.take() else {
            return;
        };
        let name = name.trim().to_owned();
        if name.is_empty() || name == self.profile_name() {
            return;
        }
        if let Some(profile) = self
            .profiles
            .iter_mut()
            .find(|profile| profile.id == self.profile)
        {
            profile.name = name.clone();
        }
        self.flash("Profile renamed", format!("now called {name}"));
        self.persist();
    }

    /// Handle a physical key press reported by the evdev monitor.
    fn phys_press(&mut self, device: &PathBuf, scancode: u16) {
        use evdev::KeyCode as K;

        let escape = scancode == K::KEY_ESC.0;
        // evdev is global: never dismiss UI from this stream. Escape is
        // handled once by the focused window's on_escape callback instead.
        if self.about_open || self.confirm_reset_mappings.is_some() {
            return;
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
        if !escape
            && self.view == View::Shortcuts
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
        if !escape
            && self.view == View::Keyboard
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
        if self.shows_input_from(device)
            && let Some(cap) = key_by_evdev(scancode)
        {
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

    fn init(mut core: Core, (): Self::Flags) -> (Self, Task<Message>) {
        // Keyloom provides its own top-level navigation and has no COSMIC
        // navigation rail. Mark the unused rail closed so the application
        // template keeps equal border padding on both sides of the content.
        core.nav_bar_set_toggled(false);

        // Tests drive the update loop against the demo state; never
        // read or write the real ~/.config from them.
        let settings = if cfg!(test) {
            None
        } else {
            KeyloomConfig::handle()
        };
        let mut stored = settings.as_ref().map(KeyloomConfig::load);
        let keyboard_layouts = stored
            .as_mut()
            .map(|stored| std::mem::take(&mut stored.keyboard_layouts))
            .unwrap_or_default();
        let setup_state = stored
            .as_ref()
            .map_or(SetupState::NotStarted, |stored| stored.setup);
        let stored = stored.filter(|stored| !stored.profiles.is_empty());

        let (profiles, profile_maps, profile, custom_profiles) = match stored {
            Some(stored) => {
                let (profiles, maps, active, custom) = stored.into_state();
                (profiles, maps, active, custom as usize)
            }
            None => {
                // Fresh install: an empty Default profile plus the
                // editable starter profiles. Once persisted they are
                // ordinary profiles like any the user creates.
                let default = Profile {
                    id: "default".to_owned(),
                    name: "Default".to_owned(),
                };
                let mut profile_maps = HashMap::from([(default.id.clone(), Vec::new())]);
                let mut profiles = vec![default];
                for (profile, maps) in model::starter_profiles() {
                    profile_maps.insert(profile.id.clone(), maps);
                    profiles.push(profile);
                }
                (profiles, profile_maps, "default".to_owned(), 0)
            }
        };
        let profile_groups: HashMap<String, Vec<Group>> = HashMap::new();

        let mut app = App {
            core,
            view: View::Keyboard,
            popover: None,
            toast: None,
            toast_seq: 0,
            remaps_open: false,
            confirm_remove_mapping: None,
            about_open: false,
            setup: None,
            setup_state,
            profiles,
            profile,
            profile_maps,
            profile_groups,
            custom_profiles,
            rename: None,
            confirm_delete: None,
            confirm_reset_mappings: None,
            undo: None,
            device: "all".to_owned(),
            form: keyboard::FORM_FULL,
            iso: false,
            keyboard_layouts,
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
            sheet_closing: None,
            devices: Vec::new(),
            monitor_started: false,
            pressed: HashSet::new(),
            last: None,
            service: None,
            grabbed: HashSet::new(),
            switching: None,
            apply_seq: 0,
            apply_pending: false,
            apply_outstanding: false,
            applying: None,
            last_apply: None,
            settings,
        };

        app.refresh_layout();
        app.set_header_title(String::new());
        // Bring the generated file in line with the loaded state, but
        // never displace a hand-written config just for launching.
        if !cfg!(test) {
            app.write_xremap(false);
        }
        // Never restart the remapper just because the app opened; the
        // next real change applies any startup rewrite along with it.
        app.apply_pending = false;
        app.apply_outstanding = false;

        // The first launch walks through system setup; afterwards it
        // waits in the menu. Tests never open it on their own.
        let mut tasks = vec![service_status_task()];
        if app.settings.is_some() && opens_setup_on_launch(app.setup_state) {
            tasks.push(app.open_setup());
        }
        (app, Task::batch(tasks))
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
                self.sheet_opened = None;
                self.sheet_closing = None;
                self.view = view;
                self.confirm_reset_mappings = None;
                self.popover = None;
                self.recording = None;
                self.remaps_open = false;
                self.confirm_remove_mapping = None;
                match view {
                    View::Keyboard => self.edit_rule = None,
                    View::Tester => {
                        self.last = None;
                        self.capture = false;
                        self.selected = None;
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
                self.rename = None;
            }
            Message::CloseOverlays => {
                self.popover = None;
                self.rename = None;
            }
            Message::SelectProfile(id) => {
                self.confirm_reset_mappings = None;
                self.profile = id;
                self.toast = None;
                self.undo = None;
                self.popover = None;
                self.rename = None;
                self.remaps_open = false;
                self.confirm_remove_mapping = None;
                self.clear_sheet();
                let toast = self.flash(
                    format!("{} profile active", self.profile_name()),
                    "switched in place",
                );
                self.persist();
                // The profile-switch confirmation dismisses itself.
                return Task::batch([
                    self.pending_apply(),
                    cosmic::task::future(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
                        Message::ToastExpired(toast)
                    }),
                ]);
            }
            Message::NewProfile { duplicate } => self.create_profile(duplicate),
            Message::DeleteProfile(id) => {
                // Only open the confirmation for profiles that could
                // actually be deleted.
                if self.view != View::Tester
                    && id != self.profile
                    && self.profiles.len() > 1
                    && self.profiles.iter().any(|profile| profile.id == id)
                {
                    self.confirm_delete = Some(id);
                }
            }
            Message::DeleteConfirm => {
                if let Some(id) = self.confirm_delete.take() {
                    self.delete_profile(&id);
                }
            }
            Message::DeleteCancel => self.confirm_delete = None,
            Message::RenameToggle => {
                if self.view != View::Tester {
                    self.rename = if self.rename.is_some() {
                        None
                    } else {
                        Some(self.profile_name().to_owned())
                    };
                    if self.rename.is_some() {
                        // Focus the rename input and select the current
                        // name so the user can edit it immediately.
                        let id = ui::overlays::rename_input_id();
                        return Task::batch([
                            cosmic::widget::text_input::focus(id.clone()),
                            cosmic::widget::text_input::select_all(id),
                        ]);
                    }
                }
            }
            Message::RenameInput(text) => {
                if self.rename.is_some() {
                    self.rename = Some(text);
                }
            }
            Message::RenameCommit => self.commit_rename(),
            Message::SelectDevice(id) => {
                if self.view == View::Tester && self.device != id {
                    self.last = None;
                }
                self.device = id;
                self.refresh_layout();
                self.popover = None;
            }
            Message::SetForm(form) => {
                self.set_layout_override(LayoutOverride {
                    form: Some(form.min(keyboard::FORM_FACTORS.len() - 1)),
                    ..self.layout_override()
                });
            }
            Message::SetVariant(iso) => {
                self.set_layout_override(LayoutOverride {
                    iso: Some(iso),
                    ..self.layout_override()
                });
            }
            Message::OpenRemaps => {
                if self.view == View::Keyboard {
                    self.remaps_open = true;
                }
            }
            Message::CloseRemaps => {
                self.remaps_open = false;
                self.confirm_remove_mapping = None;
            }
            Message::SetLayer(layer) => {
                self.layer = layer;
                if layer == Layer::Nav {
                    self.close_sheet();
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
                    if self.selected.is_none() || self.sheet_closing.is_some() {
                        // Opening (not switching keys) starts the rise; a
                        // selection during closing reverses it in place.
                        self.open_sheet();
                    }
                    self.selected = Some(code);
                    self.advanced = false;
                    self.mode = Mode::Tap;
                    self.query.clear();
                    self.category = None;
                    self.capture = false;
                    self.popover = None;
                    self.remaps_open = false;
                    self.confirm_remove_mapping = None;
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
                if self.view == View::Keyboard && self.remaps_open && self.mapping(&code).is_some()
                {
                    self.confirm_remove_mapping = Some(code);
                }
            }
            Message::RemoveMappingConfirm => {
                if let Some(code) = self.confirm_remove_mapping.take()
                    && self.view == View::Keyboard
                    && self.remaps_open
                    && self.mapping(&code).is_some()
                {
                    self.remove_mapping(&code);
                }
            }
            Message::RemoveMappingCancel => self.confirm_remove_mapping = None,
            Message::ClosePanel => {
                self.capture = false;
                self.toast = None;
                self.close_sheet();
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
                if self.edit_rule.is_none() || self.sheet_closing.is_some() {
                    self.open_sheet();
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
                    self.recording = None;
                    self.close_sheet();
                }
            }
            Message::CloseEdit => {
                self.recording = None;
                self.close_sheet();
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
                        self.sheet_opened = None;
                        self.sheet_closing = None;
                        self.toast = None;
                    }
                    Some(Undo::Profile {
                        index,
                        profile,
                        maps,
                        groups,
                    }) => {
                        self.profile_maps.insert(profile.id.clone(), maps);
                        self.profile_groups.insert(profile.id.clone(), groups);
                        self.profiles
                            .insert(index.min(self.profiles.len()), profile);
                        self.toast = None;
                        self.persist();
                    }
                    None => self.toast = None,
                }
            }
            Message::ToastExpired(id) => {
                if self.toast.as_ref().is_some_and(|toast| toast.id == id) {
                    self.toast = None;
                }
            }
            Message::Apply(seq) => match self.apply_step(seq) {
                ApplyStep::Stale => {}
                ApplyStep::Skip => self.apply_outstanding = false,
                ApplyStep::Wait(delay) => {
                    return cosmic::task::future(async move {
                        tokio::time::sleep(delay).await;
                        Message::Apply(seq)
                    });
                }
                ApplyStep::Restart => {
                    self.applying = Some(seq);
                    self.last_apply = Some(Instant::now());
                    return cosmic::task::future(async {
                        Message::Applied(service::restart().await)
                    });
                }
            },
            Message::Applied(result) => {
                // A change made during the restart keeps the applying
                // state alive: its own apply is still on the way.
                if self.applying.take() == Some(self.apply_seq) {
                    self.apply_outstanding = false;
                }
                // Success is silent — the change's own toast already
                // confirmed it. Either way, reflect the service state.
                if let Err(err) = result {
                    self.flash("Could not apply remaps", err.to_string());
                }
                return service_status_task();
            }
            Message::ServiceStatus(status) => self.service = Some(status),
            Message::SetRemapping(target) => return self.set_remapping(target),
            Message::RemappingSwitched { target, result } => {
                self.switching = None;
                match result {
                    // systemctl only reports success once the unit is
                    // in the requested state, so the chip can settle on
                    // it instead of flickering until the query lands.
                    Ok(()) => {
                        self.service = Some(match target {
                            service::Remapping::On => service::Status::Active,
                            service::Remapping::Off => service::Status::Inactive,
                        });
                    }
                    // It did not follow; the status query that comes
                    // next describes where it actually stands.
                    Err(err) => {
                        self.flash(
                            match target {
                                service::Remapping::On => "Could not resume remapping",
                                service::Remapping::Off => "Could not pause remapping",
                            },
                            err.to_string(),
                        );
                    }
                }
                return service_status_task();
            }
            // The redraw itself re-reads the animation clock. Once a close
            // reaches zero, the retained editor state can be discarded.
            Message::SheetAnimate => {
                if self.sheet_closing.is_some() && self.sheet_progress() <= f32::EPSILON {
                    self.clear_sheet();
                }
            }
            Message::MenuShowSetup => return self.open_setup(),
            Message::SetupProbed(facts) => {
                if let Some(setup) = &mut self.setup {
                    setup.probing = false;
                    setup.facts = Some(facts);
                }
            }
            Message::SetupPage(page) => {
                if let Some(setup) = &mut self.setup {
                    setup.page = page;
                }
            }
            Message::SetupRecheck => {
                if let Some(setup) = &mut self.setup
                    && !setup.probing
                {
                    setup.probing = true;
                    return Task::batch([setup_probe_task(), service_status_task()]);
                }
            }
            Message::SetupAct(step) => return self.setup_act(step),
            Message::SetupActed { step, result } => {
                // The header chip follows the service either way, even
                // when setup was closed while the fix ran.
                let Some(setup) = &mut self.setup else {
                    return service_status_task();
                };
                setup.busy = None;
                setup.error = result.err().map(|err| (step, err));
                setup.probing = true;
                return Task::batch([setup_probe_task(), service_status_task()]);
            }
            Message::SetupSkip | Message::SetupFinish => self.leave_setup(),
            Message::SetupExample => {
                if self.setup.is_some() {
                    self.apply_example();
                    self.leave_setup();
                }
            }
            Message::MenuReset => {
                self.popover = None;
                if self.view != View::Tester {
                    self.confirm_reset_mappings = Some(self.profile.clone());
                }
            }
            Message::ResetMappingsConfirm => {
                if let Some(id) = self.confirm_reset_mappings.take()
                    && id == self.profile
                    && self.view != View::Tester
                {
                    self.undo = None;
                    self.profile_maps.insert(id, Vec::new());
                    self.clear_sheet();
                    self.flash("Profile cleared", "nothing is remapped");
                    self.persist();
                }
            }
            Message::ResetMappingsCancel => self.confirm_reset_mappings = None,
            Message::MenuAbout => {
                self.popover = None;
                self.about_open = true;
            }
            Message::CloseAbout => self.about_open = false,
            Message::Monitor(event) => match event {
                monitor::Event::Started(devices) => {
                    self.devices = devices;
                    self.monitor_started = true;
                    self.pressed.clear();
                    self.refresh_layout();
                }
                monitor::Event::Connected(device) => {
                    let id = device.path.to_string_lossy().into_owned();
                    // A replugged keyboard usually returns on a new
                    // event node: follow it if its stale entry was the
                    // selected mapping scope.
                    if self.devices.iter().any(|old| {
                        !old.connected
                            && old.id == device.id
                            && old.path.to_string_lossy() == self.device
                    }) {
                        self.device = id;
                    } else if self
                        .selected_device()
                        .is_some_and(|old| old.path == device.path && old.id != device.id)
                    {
                        // An unrelated keyboard reused the selected event
                        // node. Do not silently adopt it as the selected scope.
                        self.device = "all".to_owned();
                        self.last = None;
                    }
                    // Drop the node being reused plus any stale entry
                    // for the same keyboard.
                    self.devices.retain(|old| {
                        old.path != device.path && (old.connected || old.id != device.id)
                    });
                    self.pressed.retain(|(path, _)| *path != device.path);
                    let name = device.name.clone();
                    self.devices.push(device);
                    self.devices
                        .sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
                    self.refresh_layout();
                    let toast = self.flash(
                        format!("{name} connected"),
                        "Keys light up as you type; mappings can target it.",
                    );
                    return cosmic::task::future(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
                        Message::ToastExpired(toast)
                    });
                }
                monitor::Event::Key { device, event } => match event {
                    monitor::KeyEvent::Pressed(code) => self.phys_press(&device, code),
                    monitor::KeyEvent::Repeated(_) => {}
                    monitor::KeyEvent::Released(code) => {
                        self.pressed.remove(&(device, code));
                    }
                },
                monitor::Event::Grabbed(nodes) => self.grabbed = nodes,
                monitor::Event::Disconnected(path) => {
                    if let Some(device) = self.devices.iter_mut().find(|device| device.path == path)
                    {
                        device.connected = false;
                    }
                    self.pressed.retain(|(device, _)| *device != path);
                    self.refresh_layout();
                }
            },
        }

        // Any arm that persisted a change lands here; spawn the
        // debounced apply for it.
        self.pending_apply()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![Subscription::run(monitor_stream)];
        // Drive redraws only while the sheet is actively moving.
        let sheet_progress = self.sheet_progress();
        if self.sheet_visible() && (!self.sheet_open() || sheet_progress < 1.0) {
            subscriptions.push(cosmic::iced::window::frames().map(|_| Message::SheetAnimate));
        }
        Subscription::batch(subscriptions)
    }

    /// Modal dialogs render natively above the window content.
    fn dialog(&self) -> Option<Element<'_, Message>> {
        if let Some(setup) = &self.setup {
            return Some(ui::overlays::setup_dialog(self, setup));
        }
        if self.about_open {
            return Some(ui::overlays::about_dialog());
        }
        if self.confirm_delete.is_some() {
            return Some(ui::overlays::delete_profile_dialog(self));
        }
        if self.confirm_reset_mappings.is_some() {
            return Some(ui::overlays::reset_mappings_dialog(self));
        }
        if self.confirm_remove_mapping.is_some() {
            return Some(ui::overlays::remove_mapping_dialog(self));
        }
        if self.view == View::Keyboard && self.selected.is_some() && self.capture {
            return Some(ui::overlays::capture_dialog(self));
        }
        if self.view == View::Keyboard && self.remaps_open {
            return Some(ui::overlays::remaps_dialog(self));
        }
        None
    }

    fn view(&self) -> Element<'_, Message> {
        ui::view(self)
    }

    fn on_escape(&mut self) -> Task<Message> {
        // Only window keyboard events reach this callback, so typing Escape
        // in another application cannot dismiss Keyloom's surfaces.
        // Match the dialog stacking order and dismiss only the top surface.
        if self.setup.is_some() {
            self.leave_setup();
        } else if self.about_open {
            self.about_open = false;
        } else if self.confirm_delete.is_some() {
            self.confirm_delete = None;
        } else if self.confirm_reset_mappings.is_some() {
            self.confirm_reset_mappings = None;
        } else if self.confirm_remove_mapping.is_some() {
            self.confirm_remove_mapping = None;
        } else if self.capture {
            self.capture = false;
        } else if self.remaps_open {
            self.remaps_open = false;
            self.confirm_remove_mapping = None;
        } else if self.popover.is_some() {
            self.popover = None;
            self.rename = None;
        } else if self.recording.is_some() {
            self.recording = None;
        } else {
            self.close_sheet();
        }
        Task::none()
    }
}

/// Adapts the evdev watcher into this app's message stream.
fn monitor_stream() -> impl Stream<Item = Message> + Send {
    monitor::watch().map(Message::Monitor)
}

/// One-off query of the xremap service state.
fn service_status_task() -> Task<Message> {
    cosmic::task::future(async { Message::ServiceStatus(service::status().await) })
}

/// One pass of the first-run setup checks.
fn setup_probe_task() -> Task<Message> {
    cosmic::task::future(async { Message::SetupProbed(setup::probe().await) })
}

/// Whether setup opens by itself at launch: only until the user has
/// been through it once, finished or not.
fn opens_setup_on_launch(state: SetupState) -> bool {
    state == SetupState::NotStarted
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::Application;

    fn app() -> App {
        App::init(Core::default(), ()).0
    }

    #[test]
    fn initialization_disables_the_unused_navigation_rail() {
        let app = app();

        assert!(!app.core.nav_bar_active());
    }

    #[test]
    fn global_escape_does_not_dismiss_surfaces_or_record_a_mapping() {
        let mut app = app();
        let device = PathBuf::from("/dev/input/test-keyboard");
        app.selected = Some("CapsLock");
        app.about_open = true;
        app.confirm_delete = Some("laptop".to_owned());
        app.confirm_remove_mapping = Some("CapsLock".to_owned());
        app.capture = true;
        app.remaps_open = true;
        app.setup = Some(Setup::new());
        app.popover = Some(Popover::Menu);

        for about_open in [true, false] {
            app.about_open = about_open;
            app.phys_press(&device, evdev::KeyCode::KEY_ESC.0);
            assert_eq!(app.about_open, about_open);
            assert!(app.confirm_delete.is_some());
            assert!(app.confirm_remove_mapping.is_some());
            assert!(app.capture);
            assert!(app.remaps_open);
            assert!(app.setup.is_some());
            assert!(app.popover.is_some());
            assert_eq!(app.selected, Some("CapsLock"));
            assert!(app.maps().is_empty());
        }

        app.view = View::Shortcuts;
        app.edit_rule = Some(EditRule {
            group: 0,
            rule: None,
        });
        app.recording = Some(Side::From);
        app.phys_press(&device, evdev::KeyCode::KEY_ESC.0);
        assert_eq!(app.recording, Some(Side::From));
    }

    #[test]
    fn escape_dismisses_only_about_in_either_event_order() {
        for physical_first in [true, false] {
            let mut app = app();
            let device = PathBuf::from("/dev/input/test-keyboard");
            app.selected = Some("CapsLock");
            let _ = app.update(Message::MenuAbout);
            if physical_first {
                app.phys_press(&device, evdev::KeyCode::KEY_ESC.0);
            }
            let _ = app.on_escape();
            if !physical_first {
                app.phys_press(&device, evdev::KeyCode::KEY_ESC.0);
            }
            assert!(!app.about_open);
            assert_eq!(app.selected, Some("CapsLock"));
            let _ = app.on_escape();
            assert!(app.sheet_closing.is_some());
            app.sheet_closing
                .as_mut()
                .expect("closing animation")
                .started = Instant::now()
                .checked_sub(SHEET_ANIMATION)
                .expect("animation duration fits before now");
            let _ = app.update(Message::SheetAnimate);
            assert!(app.selected.is_none());
        }
    }

    #[test]
    fn window_escape_cancels_capture_and_confirmation_before_editor() {
        let mut app = app();
        app.selected = Some("CapsLock");
        app.capture = true;
        app.confirm_delete = Some("laptop".to_owned());
        let _ = app.on_escape();
        assert!(app.confirm_delete.is_none());
        assert!(app.capture);
        let _ = app.on_escape();
        assert!(!app.capture);
        assert_eq!(app.selected, Some("CapsLock"));
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
    fn reset_requires_confirmation_and_cannot_be_undone() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("laptop".to_owned()));
        let _ = app.update(Message::SelectKey("KeyA"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        assert!(app.undo.is_some(), "an earlier edit has an undo snapshot");
        let before = app.profile_maps.clone();
        let apply_seq = app.apply_seq;
        app.popover = Some(Popover::Menu);

        let _ = app.update(Message::ResetMappingsConfirm);
        assert_eq!(
            app.profile_maps, before,
            "unsolicited confirmation is ignored"
        );
        let _ = app.update(Message::MenuReset);
        assert_eq!(app.confirm_reset_mappings.as_deref(), Some("laptop"));
        assert!(app.dialog().is_some());
        assert!(app.popover.is_none());
        assert_eq!(app.profile_maps, before);
        assert_eq!(app.apply_seq, apply_seq, "request does not apply changes");

        let _ = app.update(Message::ResetMappingsConfirm);
        assert!(app.confirm_reset_mappings.is_none());
        assert!(app.maps().is_empty());
        assert_eq!(app.apply_seq, apply_seq + 1);
        for (id, maps) in &before {
            if id != "laptop" {
                assert_eq!(app.profile_maps.get(id), Some(maps));
            }
        }
        assert!(app.undo.is_none(), "reset discards previous undo snapshots");
        let cleared = app.profile_maps.clone();
        let _ = app.update(Message::Undo);
        assert_eq!(
            app.profile_maps, cleared,
            "undo cannot restore reset mappings"
        );
    }

    #[test]
    fn cancelling_reset_preserves_mappings_editor_toast_and_undo() {
        for escape in [false, true] {
            let mut app = app();
            let _ = app.update(Message::SelectKey("CapsLock"));
            let _ = app.update(Message::PickAction("Escape".to_owned()));
            app.capture = true;
            let before = app.profile_maps.clone();
            let apply_seq = app.apply_seq;
            let toast_id = app.toast.as_ref().unwrap().id;
            let _ = app.update(Message::MenuReset);
            app.phys_press(
                &PathBuf::from("/dev/input/test-keyboard"),
                evdev::KeyCode::KEY_A.0,
            );
            if escape {
                let _ = app.on_escape();
            } else {
                // Cancel and the dialog backdrop send the same message.
                let _ = app.update(Message::ResetMappingsCancel);
            }
            assert!(app.confirm_reset_mappings.is_none());
            assert!(app.capture);
            assert_eq!(app.selected, Some("CapsLock"));
            let _ = app.update(Message::ResetMappingsConfirm);
            assert_eq!(app.profile_maps, before);
            assert_eq!(app.apply_seq, apply_seq);
            assert_eq!(app.toast.as_ref().unwrap().id, toast_id);
            let _ = app.update(Message::Undo);
            assert!(app.maps().is_empty(), "previous undo is preserved");
        }
    }

    #[test]
    fn changing_profile_or_view_cancels_pending_reset() {
        for message in [
            Message::SelectProfile("mac".to_owned()),
            Message::NewProfile { duplicate: true },
            Message::SetView(View::Tester),
            Message::SetView(View::Shortcuts),
        ] {
            let mut app = app();
            let _ = app.update(Message::SelectProfile("laptop".to_owned()));
            let _ = app.update(Message::MenuReset);
            let _ = app.update(message);
            assert!(app.confirm_reset_mappings.is_none());
            let before = app.profile_maps.clone();
            let _ = app.update(Message::ResetMappingsConfirm);
            assert_eq!(app.profile_maps, before);
        }
    }

    #[test]
    fn tester_cannot_request_a_reset() {
        let mut app = app();
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::MenuReset);
        assert!(app.confirm_reset_mappings.is_none());
    }

    #[test]
    fn profile_switch_toast_expires_by_id() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("default".to_owned()));
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
        let _ = app.update(Message::SelectProfile("default".to_owned()));
        let stale = app.toast.as_ref().unwrap().id;

        // A new mapping replaces the toast before the timer fires.
        let _ = app.update(Message::SelectKey("KeyA"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::ToastExpired(stale));

        let toast = app.toast.as_ref().expect("newer toast survives");
        assert_eq!(toast.text, "A → Escape");
    }

    #[test]
    fn changes_schedule_a_debounced_apply() {
        let mut app = app();
        assert_eq!(app.apply_seq, 0, "nothing to apply on a fresh start");

        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        assert_eq!(app.apply_seq, 1, "a mapping change schedules an apply");

        let _ = app.update(Message::OpenRemaps);
        let _ = app.update(Message::RemoveMapping("CapsLock".to_owned()));
        assert!(
            app.mapping("CapsLock").is_some(),
            "request keeps the mapping"
        );
        let _ = app.update(Message::RemoveMappingConfirm);
        assert_eq!(app.apply_seq, 2, "each change supersedes the last");
    }

    #[test]
    fn apply_restarts_only_for_the_newest_change() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let seq = app.apply_seq;

        // An apply superseded by a newer change does nothing.
        let _ = app.update(Message::Apply(seq - 1));
        assert!(app.applying.is_none());

        let _ = app.update(Message::Apply(seq));
        assert!(app.applying.is_some(), "the current apply restarts");

        // Success is silent: the mapping's own toast stays put.
        let before = app.toast.as_ref().map(|toast| toast.text.clone());
        let _ = app.update(Message::Applied(Ok(())));
        assert!(app.applying.is_none());
        assert_eq!(app.toast.as_ref().map(|toast| toast.text.clone()), before);
    }

    #[test]
    fn applying_state_spans_schedule_to_settled_restart() {
        let mut app = app();
        assert!(!app.apply_in_progress());

        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        assert!(
            app.apply_in_progress(),
            "a scheduled apply shows as applying"
        );

        let _ = app.update(Message::Apply(app.apply_seq));
        assert!(app.apply_in_progress(), "so does the restart in flight");

        let _ = app.update(Message::Applied(Ok(())));
        assert!(!app.apply_in_progress(), "settled once the restart returns");
    }

    #[test]
    fn a_change_during_the_restart_keeps_applying_shown() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::Apply(app.apply_seq));

        // A second change lands while the restart is in flight.
        let _ = app.update(Message::SelectKey("KeyA"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));

        let _ = app.update(Message::Applied(Ok(())));
        assert!(
            app.apply_in_progress(),
            "the newer change's apply is still on the way"
        );

        // Once the pacing gap has passed, the retry restarts and the
        // applying state settles with it.
        app.last_apply = None;
        let _ = app.update(Message::Apply(app.apply_seq));
        let _ = app.update(Message::Applied(Ok(())));
        assert!(!app.apply_in_progress());
    }

    #[test]
    fn applies_wait_or_skip_as_the_service_allows() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let seq = app.apply_seq;

        // While a restart is in flight, a due apply waits its turn.
        app.applying = Some(seq);
        assert!(matches!(app.apply_step(seq), ApplyStep::Wait(_)));
        app.applying = None;

        // Right after a restart, the next one waits out the gap that
        // keeps us under systemd's start rate limit.
        app.last_apply = Some(Instant::now());
        assert!(matches!(app.apply_step(seq), ApplyStep::Wait(_)));
        app.last_apply = None;

        // Stale applies are dropped; without a unit nothing can run.
        assert_eq!(app.apply_step(seq - 1), ApplyStep::Stale);
        app.service = Some(service::Status::NotFound);
        assert_eq!(app.apply_step(seq), ApplyStep::Skip);

        // A skipped apply also stops showing as in progress.
        let _ = app.update(Message::Apply(seq));
        assert!(!app.apply_in_progress());

        app.service = Some(service::Status::Active);
        assert_eq!(app.apply_step(seq), ApplyStep::Restart);
    }

    #[test]
    fn failed_apply_surfaces_the_error() {
        let mut app = app();
        let _ = app.update(Message::Applied(Err(service::Error::Refused(
            "unit not loaded".to_owned(),
        ))));

        assert!(app.applying.is_none());
        let toast = app.toast.as_ref().expect("failure is explained");
        assert_eq!(toast.text, "Could not apply remaps");
        assert_eq!(toast.sub, "unit not loaded");
    }

    #[test]
    fn service_status_is_recorded() {
        let mut app = app();
        assert_eq!(app.service, None, "state is unknown until queried");

        let _ = app.update(Message::ServiceStatus(service::Status::Active));
        assert_eq!(app.service, Some(service::Status::Active));
    }

    #[test]
    fn selection_drives_the_bottom_sheet() {
        let mut app = app();
        assert!(app.selected.is_none());

        let _ = app.update(Message::SelectKey("CapsLock"));
        assert_eq!(app.selected, Some("CapsLock"), "selecting opens the sheet");

        let _ = app.update(Message::ClosePanel);
        assert!(!app.sheet_open(), "done starts closing the sheet");
        assert!(app.sheet_visible(), "content remains during the animation");

        app.sheet_closing
            .as_mut()
            .expect("closing animation")
            .started = Instant::now()
            .checked_sub(SHEET_ANIMATION)
            .expect("animation duration fits before now");
        let _ = app.update(Message::SheetAnimate);
        assert!(app.selected.is_none(), "the final frame clears the sheet");

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
            app.sheet_progress().abs() < f32::EPSILON,
            "closed sheet reports hidden progress"
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
    fn closing_the_sheet_reverses_its_animation_before_clearing_state() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        app.sheet_opened = Instant::now().checked_sub(SHEET_ANIMATION);
        assert!((app.sheet_progress() - 1.0).abs() < f32::EPSILON);

        let _ = app.update(Message::ClosePanel);
        assert!(app.sheet_closing.is_some());
        assert!(app.sheet_progress() > 0.0);
        assert_eq!(app.selected, Some("CapsLock"));

        app.sheet_closing
            .as_mut()
            .expect("closing animation")
            .started = Instant::now()
            .checked_sub(SHEET_ANIMATION)
            .expect("animation duration fits before now");
        let _ = app.update(Message::SheetAnimate);

        assert!(!app.sheet_visible());
        assert!(app.sheet_closing.is_none());
        assert!(app.selected.is_none());
    }

    #[test]
    fn remaps_dialog_opens_and_selecting_a_row_closes_it() {
        let mut app = app();
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
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::SelectKey("KeyA"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let count = app.maps().len();
        assert!(app.mapping("CapsLock").is_some());

        let _ = app.update(Message::OpenRemaps);
        let _ = app.update(Message::RemoveMapping("CapsLock".to_owned()));
        assert!(
            app.mapping("CapsLock").is_some(),
            "request keeps the mapping"
        );
        let _ = app.update(Message::RemoveMappingConfirm);
        assert!(app.mapping("CapsLock").is_none());
        assert_eq!(app.maps().len(), count - 1);

        let _ = app.update(Message::Undo);
        assert!(app.mapping("CapsLock").is_some(), "removal is undoable");
    }

    #[test]
    fn cancelling_remap_removal_keeps_mapping_and_list_open() {
        for escape in [false, true] {
            let mut app = app();
            let _ = app.update(Message::SelectKey("CapsLock"));
            let _ = app.update(Message::PickAction("Escape".to_owned()));
            let _ = app.update(Message::OpenRemaps);
            let apply_seq = app.apply_seq;
            let toast_id = app.toast.as_ref().unwrap().id;
            let _ = app.update(Message::RemoveMapping("CapsLock".to_owned()));
            assert_eq!(app.confirm_remove_mapping.as_deref(), Some("CapsLock"));
            assert_eq!(app.apply_seq, apply_seq, "request does not apply changes");

            if escape {
                let _ = app.on_escape();
            } else {
                // Cancel and the confirmation's backdrop share this message.
                let _ = app.update(Message::RemoveMappingCancel);
            }
            assert!(app.confirm_remove_mapping.is_none());
            assert!(app.remaps_open);
            assert!(app.mapping("CapsLock").is_some());
            assert_eq!(app.apply_seq, apply_seq);
            assert_eq!(app.toast.as_ref().unwrap().id, toast_id);

            let _ = app.update(Message::RemoveMappingConfirm);
            assert!(
                app.mapping("CapsLock").is_some(),
                "stale confirmation is ignored"
            );
            let _ = app.update(Message::Undo);
            assert!(
                app.mapping("CapsLock").is_none(),
                "previous undo is preserved"
            );
        }
    }

    #[test]
    fn confirming_last_remap_removal_keeps_empty_list_open() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::OpenRemaps);
        let _ = app.update(Message::RemoveMapping("CapsLock".to_owned()));
        let _ = app.update(Message::RemoveMappingConfirm);
        assert!(app.confirm_remove_mapping.is_none());
        assert!(app.remaps_open);
        assert!(app.maps().is_empty());
        assert!(app.dialog().is_some());
        let _ = app.update(Message::Undo);
        assert!(app.mapping("CapsLock").is_some());
    }

    #[test]
    fn leaving_remaps_clears_pending_removal() {
        for message in [
            Message::CloseRemaps,
            Message::SelectKey("KeyA"),
            Message::SetView(View::Tester),
            Message::SetView(View::Shortcuts),
            Message::SelectProfile("laptop".to_owned()),
            Message::NewProfile { duplicate: true },
        ] {
            let mut app = app();
            let _ = app.update(Message::SelectKey("CapsLock"));
            let _ = app.update(Message::PickAction("Escape".to_owned()));
            let _ = app.update(Message::OpenRemaps);
            let _ = app.update(Message::RemoveMapping("CapsLock".to_owned()));
            let _ = app.update(message);
            assert!(app.confirm_remove_mapping.is_none());
            let before = app.profile_maps.clone();
            let _ = app.update(Message::RemoveMappingConfirm);
            assert_eq!(app.profile_maps, before);
        }
    }

    #[test]
    fn remap_removal_requires_an_existing_mapping_and_open_list() {
        let mut app = app();
        let _ = app.update(Message::OpenRemaps);
        let _ = app.update(Message::RemoveMapping("missing".to_owned()));
        assert!(app.confirm_remove_mapping.is_none());

        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::RemoveMapping("CapsLock".to_owned()));
        assert!(
            app.confirm_remove_mapping.is_none(),
            "closed list ignores removal"
        );

        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::RemoveMapping("CapsLock".to_owned()));
        let _ = app.update(Message::RemoveMappingConfirm);
        assert!(app.confirm_remove_mapping.is_none());
        assert!(app.mapping("CapsLock").is_some());
    }

    #[test]
    fn switching_decks_preserves_mappings_and_yaml() {
        let mut app = app();
        let _ = app.update(started(keyboard::FORM_TKL, true));
        let _ = app.update(connected(
            "/dev/input/event1",
            "Laptop",
            keyboard::FORM_SIXTY_FIVE,
            false,
        ));
        let _ = app.update(Message::SelectKey("Numpad7"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let yaml = crate::xremap::generate(app.maps(), |id| id.to_owned());
        let apply_seq = app.apply_seq;

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
        for scope in ["/dev/input/event0", "/dev/input/event1", "all"] {
            let _ = app.update(Message::SelectDevice(scope.to_owned()));
            let _ = app.update(Message::SetForm(app.detected_form().unwrap()));
            let _ = app.update(Message::SetVariant(app.detected_iso().unwrap()));
            assert_eq!(
                crate::xremap::generate(app.maps(), |id| id.to_owned()),
                yaml
            );
        }
        assert_eq!(
            app.apply_seq, apply_seq,
            "layout changes do not restart the backend"
        );
    }

    fn test_device(
        path: &str,
        name: &str,
        form: usize,
        form_hinted: bool,
    ) -> monitor::KeyboardDevice {
        monitor::KeyboardDevice {
            path: PathBuf::from(path),
            id: monitor::KeyboardId::new(
                evdev::InputId::new(evdev::BusType::BUS_USB, 1, 1, 1),
                Some(name),
                None,
                name,
            ),
            name: name.to_owned(),
            connected: true,
            form,
            form_hinted,
            iso: false,
            virtual_device: false,
        }
    }

    fn started(form: usize, form_hinted: bool) -> Message {
        Message::Monitor(monitor::Event::Started(vec![test_device(
            "/dev/input/event0",
            "Test Keyboard",
            form,
            form_hinted,
        )]))
    }

    #[test]
    fn detected_size_and_variant_default_the_deck_without_saving_overrides() {
        let mut app = app();
        assert_eq!((app.form, app.iso), (keyboard::FORM_FULL, false));

        let mut device = test_device("/dev/input/event0", "ISO board", keyboard::FORM_TKL, true);
        device.iso = true;
        let _ = app.update(Message::Monitor(monitor::Event::Started(vec![device])));
        for scope in ["all", "/dev/input/event0"] {
            let _ = app.update(Message::SelectDevice(scope.to_owned()));
            assert_eq!((app.form, app.iso), (keyboard::FORM_TKL, true));
            let _ = app.update(Message::TogglePopover(Popover::Size));
            assert_eq!(app.layout_override(), LayoutOverride::default());
        }
        assert_eq!(app.keyboard_layouts, KeyboardLayouts::default());
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

    fn connected(path: &str, name: &str, form: usize, form_hinted: bool) -> Message {
        Message::Monitor(monitor::Event::Connected(test_device(
            path,
            name,
            form,
            form_hinted,
        )))
    }

    #[test]
    fn selected_keyboard_uses_its_own_detection_in_keyboard_and_tester_views() {
        for view in [View::Keyboard, View::Tester] {
            let mut app = app();
            let _ = app.update(Message::SetView(view));
            let mut iso_keyboard = test_device(
                "/dev/input/event0",
                "Test Keyboard",
                keyboard::FORM_TKL,
                true,
            );
            iso_keyboard.iso = true;
            let _ = app.update(Message::Monitor(monitor::Event::Started(vec![
                iso_keyboard,
            ])));
            let _ = app.update(connected(
                "/dev/input/event1",
                "Laptop",
                keyboard::FORM_FULL,
                false,
            ));
            assert_eq!(
                app.form,
                keyboard::FORM_TKL,
                "All keyboards prefers the name hint"
            );
            assert!(app.iso, "All keyboards includes the connected ISO key");

            let _ = app.update(Message::SelectDevice("/dev/input/event1".to_owned()));
            assert_eq!(app.detected_form(), Some(keyboard::FORM_FULL));
            assert_eq!((app.form, app.iso), (keyboard::FORM_FULL, false));
            let _ = app.update(Message::SelectDevice("/dev/input/event0".to_owned()));
            assert_eq!((app.form, app.iso), (keyboard::FORM_TKL, true));
            let _ = app.update(connected(
                "/dev/input/event2",
                "New keyboard",
                keyboard::FORM_FULL,
                true,
            ));
            assert_eq!(
                app.form,
                keyboard::FORM_TKL,
                "unrelated hotplug leaves the selected deck alone"
            );
        }
    }

    #[test]
    fn size_and_variant_overrides_are_independent_per_keyboard_and_all_scope() {
        let mut app = app();
        let _ = app.update(started(keyboard::FORM_TKL, true));
        let _ = app.update(connected(
            "/dev/input/event1",
            "Laptop",
            keyboard::FORM_FULL,
            false,
        ));
        let _ = app.update(Message::SetForm(keyboard::FORM_SEVENTY_FIVE));
        let _ = app.update(Message::SetVariant(true));

        let _ = app.update(Message::SelectDevice("/dev/input/event0".to_owned()));
        assert_eq!((app.form, app.iso), (keyboard::FORM_TKL, false));
        let _ = app.update(Message::SetForm(keyboard::FORM_SIXTY_FIVE));
        let _ = app.update(Message::SetVariant(true));
        let _ = app.update(Message::SelectDevice("/dev/input/event1".to_owned()));
        assert_eq!((app.form, app.iso), (keyboard::FORM_FULL, false));
        let _ = app.update(Message::SetForm(keyboard::FORM_SIXTY));

        for (scope, form, iso) in [
            ("/dev/input/event0", keyboard::FORM_SIXTY_FIVE, true),
            ("all", keyboard::FORM_SEVENTY_FIVE, true),
            ("/dev/input/event1", keyboard::FORM_SIXTY, false),
        ] {
            let _ = app.update(Message::SetView(View::Tester));
            let _ = app.update(Message::SelectDevice(scope.to_owned()));
            assert_eq!((app.form, app.iso), (form, iso));
            let _ = app.update(Message::SetView(View::Keyboard));
            assert_eq!((app.form, app.iso), (form, iso));
        }
        assert_eq!(
            app.apply_seq, 0,
            "display settings do not schedule remap applies"
        );
    }

    #[test]
    fn choosing_detected_values_changes_only_the_selected_scope_and_axis() {
        let mut app = app();
        let _ = app.update(started(keyboard::FORM_TKL, true));
        app.devices[0].iso = true;
        app.refresh_layout();
        let _ = app.update(Message::SetForm(keyboard::FORM_FULL));
        let _ = app.update(Message::SelectDevice("/dev/input/event0".to_owned()));
        let _ = app.update(Message::SetForm(keyboard::FORM_SIXTY));
        let _ = app.update(Message::SetVariant(false));

        let _ = app.update(Message::SetForm(keyboard::FORM_TKL));
        assert_eq!((app.form, app.iso), (keyboard::FORM_TKL, false));
        assert_eq!(app.layout_override().form, Some(keyboard::FORM_TKL));
        assert_eq!(app.layout_override().iso, Some(false));
        let _ = app.update(Message::SetVariant(true));
        assert!(app.iso);
        assert_eq!(app.layout_override().iso, Some(true));
        let _ = app.update(Message::SelectDevice("all".to_owned()));
        assert_eq!(
            app.form,
            keyboard::FORM_FULL,
            "All keyboards keeps its override"
        );
        let _ = app.update(Message::SetForm(keyboard::FORM_TKL));
        assert_eq!(app.form, keyboard::FORM_TKL);
        let _ = app.update(connected(
            "/dev/input/event1",
            "Full board",
            keyboard::FORM_FULL,
            true,
        ));
        assert_eq!(
            app.form,
            keyboard::FORM_TKL,
            "an explicit choice remains selected even when detection changes"
        );
    }

    #[test]
    fn disconnected_selection_keeps_its_layout_while_all_scope_recalculates() {
        let mut app = app();
        let _ = app.update(started(keyboard::FORM_SIXTY, true));
        let _ = app.update(connected(
            "/dev/input/event1",
            "Full board",
            keyboard::FORM_FULL,
            true,
        ));
        let _ = app.update(Message::SelectDevice("/dev/input/event1".to_owned()));
        let _ = app.update(Message::Monitor(monitor::Event::Disconnected(
            PathBuf::from("/dev/input/event1"),
        )));
        assert_eq!(app.form, keyboard::FORM_FULL);
        let _ = app.update(Message::SelectDevice("all".to_owned()));
        assert_eq!(app.form, keyboard::FORM_SIXTY);
        let _ = app.update(connected(
            "/dev/input/event1",
            "Full board",
            keyboard::FORM_FULL,
            true,
        ));
        assert_eq!(app.form, keyboard::FORM_FULL);
        let _ = app.update(Message::Monitor(monitor::Event::Disconnected(
            PathBuf::from("/dev/input/event1"),
        )));
        assert_eq!(
            app.form,
            keyboard::FORM_SIXTY,
            "All keyboards follows disconnections"
        );
        let _ = app.update(Message::Monitor(monitor::Event::Disconnected(
            PathBuf::from("/dev/input/event0"),
        )));
        assert_eq!(app.detected_form(), None);
        assert_eq!(
            app.form,
            keyboard::FORM_FULL,
            "no connected keyboards uses the default"
        );
    }

    #[test]
    fn reconnect_restores_overrides_without_confusing_same_named_keyboards() {
        let mut app = app();
        let mut first = test_device("/dev/input/event0", "serial-a", keyboard::FORM_TKL, true);
        let mut second = test_device("/dev/input/event1", "serial-b", keyboard::FORM_TKL, true);
        first.name = "Same model".to_owned();
        second.name = first.name.clone();
        let _ = app.update(Message::Monitor(monitor::Event::Started(vec![
            first.clone(),
            second.clone(),
        ])));
        let _ = app.update(Message::SelectDevice("/dev/input/event0".to_owned()));
        let _ = app.update(Message::SetForm(keyboard::FORM_SIXTY));
        let _ = app.update(Message::SetVariant(true));
        let _ = app.update(Message::Monitor(monitor::Event::Disconnected(
            first.path.clone(),
        )));
        let _ = app.update(Message::Monitor(monitor::Event::Disconnected(
            second.path.clone(),
        )));
        second.path = PathBuf::from("/dev/input/event7");
        let _ = app.update(Message::Monitor(monitor::Event::Connected(second)));
        assert_eq!(
            app.device, "/dev/input/event0",
            "same name does not steal the selection"
        );
        assert_eq!((app.form, app.iso), (keyboard::FORM_SIXTY, true));
        first.path = PathBuf::from("/dev/input/event8");
        let _ = app.update(Message::Monitor(monitor::Event::Connected(first)));
        assert_eq!(app.device, "/dev/input/event8");
        assert_eq!((app.form, app.iso), (keyboard::FORM_SIXTY, true));
        assert_eq!(app.devices.len(), 2);
        let _ = app.update(Message::SelectDevice("/dev/input/event7".to_owned()));
        assert_eq!((app.form, app.iso), (keyboard::FORM_TKL, false));
    }

    #[test]
    fn unrelated_keyboard_reusing_event_node_does_not_inherit_override_or_selection() {
        let mut app = app();
        let _ = app.update(started(keyboard::FORM_TKL, true));
        let _ = app.update(Message::SelectDevice("/dev/input/event0".to_owned()));
        let _ = app.update(Message::SetForm(keyboard::FORM_SIXTY));
        let _ = app.update(Message::Monitor(monitor::Event::Disconnected(
            PathBuf::from("/dev/input/event0"),
        )));
        let _ = app.update(connected(
            "/dev/input/event0",
            "Different board",
            keyboard::FORM_FULL,
            true,
        ));
        assert_eq!(app.device, "all");
        let _ = app.update(Message::SelectDevice("/dev/input/event0".to_owned()));
        assert_eq!(app.form, keyboard::FORM_FULL);
        assert_eq!(app.layout_override().form, None);
    }

    #[test]
    fn invalid_saved_size_falls_back_to_detection() {
        let mut app = app();
        app.keyboard_layouts.all.form = Some(usize::MAX);
        let _ = app.update(started(keyboard::FORM_TKL, true));
        assert_eq!(app.form, keyboard::FORM_TKL);
        assert_eq!(app.layout_override().form, None);
        assert!(!app.deck().is_empty());
    }

    #[test]
    fn saved_layouts_restore_on_new_event_nodes_without_changing_profile_storage() {
        let dir = std::env::temp_dir().join(format!("keyloom-layouts-test-{}", std::process::id()));
        let handle = cosmic_config::Config::with_custom_path(
            config::APP_ID,
            KeyloomConfig::VERSION,
            dir.clone(),
        )
        .unwrap();
        let mut original = app();
        let snapshot = KeyloomConfig::snapshot(
            &original.profiles,
            &original.profile_maps,
            &original.profile,
            0,
            &original.keyboard_layouts,
            original.setup_state,
        );
        snapshot.write_entry(&handle).unwrap();
        original.settings = Some(handle);
        let _ = original.update(started(keyboard::FORM_TKL, true));
        let _ = original.update(Message::SetForm(keyboard::FORM_SEVENTY_FIVE));
        let _ = original.update(Message::SelectDevice("/dev/input/event0".to_owned()));
        let _ = original.update(Message::SetForm(keyboard::FORM_SIXTY_FIVE));
        let _ = original.update(Message::SetVariant(true));
        assert_eq!(original.apply_seq, 0);

        let stored = KeyloomConfig::load(original.settings.as_ref().unwrap());
        assert_eq!(stored.profiles, snapshot.profiles);
        assert_eq!(stored.active_profile, snapshot.active_profile);
        let mut restored = app();
        restored.keyboard_layouts = stored.keyboard_layouts;
        let _ = restored.update(Message::Monitor(monitor::Event::Started(vec![
            test_device(
                "/dev/input/event9",
                "Test Keyboard",
                keyboard::FORM_TKL,
                true,
            ),
        ])));
        assert_eq!(
            restored.form,
            keyboard::FORM_SEVENTY_FIVE,
            "All keyboards override survives reload"
        );
        let _ = restored.update(Message::SelectDevice("/dev/input/event9".to_owned()));
        assert_eq!(
            (restored.form, restored.iso),
            (keyboard::FORM_SIXTY_FIVE, true)
        );

        restored.settings = original.settings.take();
        let _ = restored.update(Message::SetForm(keyboard::FORM_TKL));
        let _ = restored.update(Message::SetVariant(false));
        let stored = KeyloomConfig::load(restored.settings.as_ref().unwrap());
        assert_eq!(
            stored.keyboard_layouts.devices.get(&restored.devices[0].id),
            Some(&LayoutOverride {
                form: Some(keyboard::FORM_TKL),
                iso: Some(false),
            }),
            "choosing the detected values persists them like any manual choice"
        );
        assert_eq!(
            stored.keyboard_layouts.all.form,
            Some(keyboard::FORM_SEVENTY_FIVE)
        );
        assert_eq!(stored.profiles, snapshot.profiles);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hotplugged_keyboard_joins_the_device_list() {
        let mut app = app();
        let _ = app.update(started(keyboard::FORM_SIXTY, true));
        assert_eq!(app.devices.len(), 1);
        assert_eq!(app.form, keyboard::FORM_SIXTY);

        let _ = app.update(connected(
            "/dev/input/event5",
            "Desk Board",
            keyboard::FORM_FULL,
            true,
        ));
        assert_eq!(app.devices.len(), 2, "the new keyboard is listed");
        assert_eq!(app.device_entries().len(), 3, "and offered as a scope");
        assert_eq!(
            app.form,
            keyboard::FORM_FULL,
            "detection re-runs for the new keyboard"
        );
        assert!(app.toast.is_some(), "the connection is announced");
    }

    #[test]
    fn replugged_keyboard_replaces_its_stale_entry() {
        let mut app = app();
        let _ = app.update(started(keyboard::FORM_FULL, true));
        let _ = app.update(Message::SelectDevice("/dev/input/event0".to_owned()));
        let _ = app.update(Message::Monitor(monitor::Event::Disconnected(
            PathBuf::from("/dev/input/event0"),
        )));
        assert!(!app.devices[0].connected);

        // The same keyboard returns on a different event node.
        let _ = app.update(connected(
            "/dev/input/event7",
            "Test Keyboard",
            keyboard::FORM_FULL,
            true,
        ));
        assert_eq!(app.devices.len(), 1, "no duplicate disconnected entry");
        assert!(app.devices[0].connected);
        assert_eq!(app.devices[0].path, PathBuf::from("/dev/input/event7"));
        assert_eq!(
            app.device, "/dev/input/event7",
            "the selected scope follows the replugged keyboard"
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
    fn fresh_install_seeds_default_and_starter_profiles() {
        let app = app();
        let starters = model::starter_profiles();
        assert_eq!(app.profiles.len(), 1 + starters.len());
        assert_eq!(app.profile_name(), "Default", "the empty profile is active");
        assert!(app.maps().is_empty(), "no mappings apply out of the box");
        assert!(app.groups().is_empty(), "no demo groups are seeded");
        assert!(app.devices.is_empty(), "no demo devices are listed");
        assert_eq!(app.device_entries().len(), 1, "only the All keyboards row");
        for (profile, maps) in &starters {
            assert!(
                app.profiles.iter().any(|seeded| seeded.id == profile.id),
                "{} ships as a regular profile",
                profile.name
            );
            assert_eq!(app.profile_maps.get(&profile.id), Some(maps));
        }
    }

    #[test]
    fn starter_profiles_are_editable_like_any_other() {
        let mut app = app();
        let seeded = model::starter_profiles()
            .into_iter()
            .find(|(profile, _)| profile.id == "laptop")
            .map(|(_, maps)| maps)
            .expect("laptop profile ships");

        let _ = app.update(Message::SelectProfile("laptop".to_owned()));
        assert_eq!(app.profile_name(), "Laptop");
        assert_eq!(app.maps(), &seeded);

        // Editing changes the profile itself; nothing is copied.
        let _ = app.update(Message::SelectKey("KeyA"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        assert_eq!(app.profiles.len(), 1 + model::starter_profiles().len());
        assert_eq!(app.maps().len(), seeded.len() + 1);

        // Renaming works in place, like any profile.
        let _ = app.update(Message::RenameToggle);
        let _ = app.update(Message::RenameInput("Travel".to_owned()));
        let _ = app.update(Message::RenameCommit);
        assert_eq!(app.profile_name(), "Travel");
    }

    #[test]
    fn deleting_a_profile_asks_for_confirmation_first() {
        let mut app = app();
        let before = app.profiles.len();

        // The request only opens the dialog; nothing is deleted yet.
        let _ = app.update(Message::DeleteProfile("laptop".to_owned()));
        assert_eq!(app.confirm_delete.as_deref(), Some("laptop"));
        assert_eq!(app.profiles.len(), before);

        // Cancelling keeps the profile.
        let _ = app.update(Message::DeleteCancel);
        assert!(app.confirm_delete.is_none());
        assert_eq!(app.profiles.len(), before);

        // Confirming deletes it, and undo restores it in place.
        let _ = app.update(Message::DeleteProfile("laptop".to_owned()));
        let _ = app.update(Message::DeleteConfirm);
        assert!(app.confirm_delete.is_none());
        assert_eq!(app.profiles.len(), before - 1);
        assert!(!app.profiles.iter().any(|profile| profile.id == "laptop"));
        assert!(!app.profile_maps.contains_key("laptop"));
        assert_eq!(app.profile, "default", "the active profile is untouched");

        let _ = app.update(Message::Undo);
        assert_eq!(app.profiles.len(), before);
        assert_eq!(app.profiles[1].id, "laptop", "undo restores its position");
        assert!(
            !app.profile_maps["laptop"].is_empty(),
            "undo restores its mappings"
        );
    }

    #[test]
    fn the_active_and_last_profiles_cannot_be_deleted() {
        let mut app = app();
        let before = app.profiles.len();

        // The active profile is protected: no confirmation opens.
        let _ = app.update(Message::DeleteProfile("default".to_owned()));
        assert!(app.confirm_delete.is_none());
        assert_eq!(app.profiles.len(), before);

        // The tester never edits state.
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::DeleteProfile("laptop".to_owned()));
        assert!(app.confirm_delete.is_none());
        assert_eq!(app.profiles.len(), before);
        let _ = app.update(Message::SetView(View::Keyboard));

        // Deleting every inactive profile leaves the active one.
        for id in ["laptop", "mac", "gaming", "media"] {
            let _ = app.update(Message::DeleteProfile(id.to_owned()));
            let _ = app.update(Message::DeleteConfirm);
        }
        assert_eq!(app.profiles.len(), 1);
        let _ = app.update(Message::DeleteProfile("default".to_owned()));
        assert!(app.confirm_delete.is_none());
        let _ = app.update(Message::DeleteConfirm);
        assert_eq!(app.profiles.len(), 1, "at least one profile remains");
        assert_eq!(app.profile_name(), "Default");
    }

    #[test]
    fn renaming_the_active_profile() {
        let mut app = app();
        let _ = app.update(Message::RenameToggle);
        assert_eq!(app.rename.as_deref(), Some("Default"));

        let _ = app.update(Message::RenameInput("Typing".to_owned()));
        let _ = app.update(Message::RenameCommit);
        assert_eq!(app.profile_name(), "Typing");
        assert!(app.rename.is_none());

        // A blank name cancels instead of renaming.
        let _ = app.update(Message::RenameToggle);
        let _ = app.update(Message::RenameInput("   ".to_owned()));
        let _ = app.update(Message::RenameCommit);
        assert_eq!(app.profile_name(), "Typing");

        // The tester never edits state.
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::RenameToggle);
        assert!(app.rename.is_none());
    }

    /// A system where every setup step is in order.
    fn ready_facts() -> setup::Facts {
        setup::Facts {
            user: Some("me".to_owned()),
            xremap: setup::XremapCheck::Found {
                path: PathBuf::from("/usr/bin/xremap"),
                version: Some("0.15.12".to_owned()),
            },
            group: setup::GroupCheck::Effective,
            uinput: setup::UinputCheck::Writable,
            unit: setup::UnitCheck::Keyloom {
                active: true,
                enabled: true,
            },
            config: Some(PathBuf::from("/home/me/.config/xremap/keyloom.yml")),
        }
    }

    /// A fresh system: xremap installed, nothing else done.
    fn fresh_facts() -> setup::Facts {
        setup::Facts {
            group: setup::GroupCheck::NotMember,
            uinput: setup::UinputCheck::NotWritable {
                rule_installed: false,
            },
            unit: setup::UnitCheck::Missing,
            ..ready_facts()
        }
    }

    #[test]
    fn setup_opens_only_on_the_first_launch() {
        assert!(opens_setup_on_launch(SetupState::NotStarted));
        assert!(!opens_setup_on_launch(SetupState::Deferred));
        assert!(!opens_setup_on_launch(SetupState::Complete));
        // Tests have no settings store, so init never opens it here.
        assert!(app().setup.is_none());
    }

    #[test]
    fn setup_opens_from_the_menu_and_walks_its_pages() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::TogglePopover(Popover::Menu));
        let _ = app.update(Message::MenuShowSetup);
        let setup = app.setup.as_ref().expect("setup opens");
        assert_eq!(setup.page, SetupPage::Welcome);
        assert!(setup.probing, "opening starts the checks");
        assert!(setup.facts.is_none());
        assert!(app.popover.is_none());
        assert!(app.selected.is_none(), "the editor sheet gives way");
        assert!(app.dialog().is_some());

        let _ = app.update(Message::SetupProbed(fresh_facts()));
        let setup = app.setup.as_ref().unwrap();
        assert!(!setup.probing);
        assert_eq!(setup.facts, Some(fresh_facts()));

        for page in [
            SetupPage::Step(setup::Step::Xremap),
            SetupPage::Step(setup::Step::Service),
            SetupPage::Finish,
            SetupPage::Welcome,
        ] {
            let _ = app.update(Message::SetupPage(page));
            assert_eq!(app.setup.as_ref().unwrap().page, page);
            assert!(app.dialog().is_some());
        }

        // Rechecking runs the probe once at a time.
        let _ = app.update(Message::SetupRecheck);
        assert!(app.setup.as_ref().unwrap().probing);
        let _ = app.update(Message::SetupRecheck);
        assert!(app.setup.as_ref().unwrap().probing);
        let _ = app.update(Message::SetupProbed(ready_facts()));
        assert!(!app.setup.as_ref().unwrap().probing);
    }

    #[test]
    fn leaving_setup_records_completion_only_when_the_system_is_ready() {
        // Skipped before the checks landed: come back later.
        let mut app = app();
        let _ = app.update(Message::MenuShowSetup);
        let _ = app.update(Message::SetupSkip);
        assert!(app.setup.is_none());
        assert_eq!(app.setup_state, SetupState::Deferred);

        // Unfinished, however it is closed.
        let _ = app.update(Message::MenuShowSetup);
        let _ = app.update(Message::SetupProbed(fresh_facts()));
        let _ = app.update(Message::SetupPage(SetupPage::Finish));
        let _ = app.update(Message::SetupFinish);
        assert_eq!(app.setup_state, SetupState::Deferred);

        // Everything in order: complete, even when skipped.
        let _ = app.update(Message::MenuShowSetup);
        let _ = app.update(Message::SetupProbed(ready_facts()));
        let _ = app.update(Message::SetupSkip);
        assert_eq!(app.setup_state, SetupState::Complete);

        // Waiting only for a new login counts as complete too.
        let mut app = self::app();
        let _ = app.update(Message::MenuShowSetup);
        let mut waiting = ready_facts();
        waiting.group = setup::GroupCheck::NeedsLogin;
        waiting.unit = setup::UnitCheck::Keyloom {
            active: false,
            enabled: true,
        };
        let _ = app.update(Message::SetupProbed(waiting));
        let _ = app.on_escape();
        assert!(app.setup.is_none(), "Escape closes setup");
        assert_eq!(app.setup_state, SetupState::Complete);

        // Reopening a completed setup never makes it incomplete again.
        let _ = app.update(Message::MenuShowSetup);
        let _ = app.update(Message::SetupProbed(fresh_facts()));
        let _ = app.update(Message::SetupSkip);
        assert_eq!(app.setup_state, SetupState::Complete);
    }

    #[test]
    fn setup_fixes_run_one_at_a_time_and_report_failures() {
        let mut app = app();
        let _ = app.update(Message::MenuShowSetup);

        // Nothing to act on before the checks land.
        let _ = app.update(Message::SetupAct(setup::Step::InputGroup));
        assert_eq!(app.setup.as_ref().unwrap().busy, None);

        let _ = app.update(Message::SetupProbed(fresh_facts()));
        // xremap has no fix, only a recheck.
        let _ = app.update(Message::SetupAct(setup::Step::Xremap));
        assert_eq!(app.setup.as_ref().unwrap().busy, None);

        let _ = app.update(Message::SetupAct(setup::Step::InputGroup));
        assert_eq!(
            app.setup.as_ref().unwrap().busy,
            Some(setup::Step::InputGroup)
        );
        // A second fix waits for the first.
        let _ = app.update(Message::SetupAct(setup::Step::Uinput));
        assert_eq!(
            app.setup.as_ref().unwrap().busy,
            Some(setup::Step::InputGroup)
        );

        let _ = app.update(Message::SetupActed {
            step: setup::Step::InputGroup,
            result: Err(setup::ActionError::Cancelled),
        });
        let setup = app.setup.as_ref().unwrap();
        assert_eq!(setup.busy, None);
        assert_eq!(
            setup.error,
            Some((setup::Step::InputGroup, setup::ActionError::Cancelled))
        );
        assert!(setup.probing, "the outcome is checked, not assumed");

        // The next attempt clears the old failure.
        let _ = app.update(Message::SetupProbed(fresh_facts()));
        let _ = app.update(Message::SetupAct(setup::Step::Uinput));
        let setup = app.setup.as_ref().unwrap();
        assert_eq!(setup.busy, Some(setup::Step::Uinput));
        assert_eq!(setup.error, None);
        let _ = app.update(Message::SetupActed {
            step: setup::Step::Uinput,
            result: Ok(()),
        });
        assert_eq!(app.setup.as_ref().unwrap().error, None);

        // The service step acts only when the facts offer something.
        let _ = app.update(Message::SetupProbed(ready_facts()));
        let _ = app.update(Message::SetupAct(setup::Step::Service));
        assert_eq!(app.setup.as_ref().unwrap().busy, None);
        let _ = app.update(Message::SetupProbed(fresh_facts()));
        let _ = app.update(Message::SetupAct(setup::Step::Service));
        assert_eq!(app.setup.as_ref().unwrap().busy, Some(setup::Step::Service));

        // A result arriving after setup was closed is simply dropped.
        let _ = app.update(Message::SetupSkip);
        let _ = app.update(Message::SetupActed {
            step: setup::Step::Service,
            result: Ok(()),
        });
        assert!(app.setup.is_none());
    }

    #[test]
    fn the_setup_example_maps_caps_lock_to_escape_and_closes() {
        let mut app = app();
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::MenuShowSetup);
        let _ = app.update(Message::SetupProbed(ready_facts()));
        let _ = app.update(Message::SetupPage(SetupPage::Finish));
        let _ = app.update(Message::SetupExample);

        assert!(app.setup.is_none());
        assert_eq!(app.setup_state, SetupState::Complete);
        assert_eq!(app.view, View::Keyboard, "the new remap is shown");
        assert_eq!(
            app.mapping("CapsLock").and_then(|m| m.tap.as_deref()),
            Some("Escape")
        );
        assert_eq!(app.apply_seq, 1, "the example applies like any change");
        let _ = app.update(Message::Undo);
        assert!(app.mapping("CapsLock").is_none(), "and is undoable");

        // Outside setup the message does nothing.
        let _ = app.update(Message::SetupExample);
        assert!(app.mapping("CapsLock").is_none());
    }

    #[test]
    fn remap_view_does_not_mirror_keys_but_tester_and_capture_still_work() {
        use evdev::KeyCode as K;
        let first = PathBuf::from("/dev/input/event0");
        let second = PathBuf::from("/dev/input/event1");

        for scope in ["all", "/dev/input/event0"] {
            let mut app = app();
            let _ = app.update(Message::SelectDevice(scope.to_owned()));
            let _ = app.update(Message::SelectKey("CapsLock"));
            app.phys_press(&first, K::KEY_A.0);
            app.phys_press(&second, K::KEY_B.0);

            assert!(!app.highlights_key(K::KEY_A.0));
            assert!(!app.highlights_key(K::KEY_B.0));
            assert_eq!(app.selected, Some("CapsLock"));
            assert!(app.maps().is_empty());

            let _ = app.update(Message::SetView(View::Tester));
            assert!(app.highlights_key(K::KEY_A.0));
            assert_eq!(app.highlights_key(K::KEY_B.0), scope == "all");
            let _ = app.update(Message::Monitor(monitor::Event::Key {
                device: first.clone(),
                event: monitor::KeyEvent::Released(K::KEY_A.0),
            }));
            assert!(!app.highlights_key(K::KEY_A.0));

            let _ = app.update(Message::SetView(View::Keyboard));
            assert!(!app.highlights_key(K::KEY_B.0));
            let _ = app.update(Message::SelectKey("CapsLock"));
            let _ = app.update(Message::SetCapture(true));
            app.phys_press(&first, K::KEY_C.0);
            assert!(!app.capture);
            assert_eq!(
                app.mapping("CapsLock")
                    .and_then(|mapping| mapping.tap.as_deref()),
                Some("C")
            );
            assert!(!app.highlights_key(K::KEY_C.0));
        }
    }

    #[test]
    fn tester_filters_physical_keys_and_modifiers_by_device() {
        use evdev::KeyCode as K;
        let mut app = app();
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::SelectDevice("/dev/input/event0".to_owned()));
        let key = |device: &str, event| {
            Message::Monitor(monitor::Event::Key {
                device: PathBuf::from(device),
                event,
            })
        };

        let _ = app.update(key(
            "/dev/input/event1",
            monitor::KeyEvent::Pressed(K::KEY_A.0),
        ));
        let _ = app.update(key(
            "/dev/input/event1",
            monitor::KeyEvent::Pressed(K::KEY_LEFTSHIFT.0),
        ));
        assert!(app.last.is_none());
        assert!(!app.is_pressed(K::KEY_A.0));
        assert_eq!(app.held_mods(), [false; 4]);

        let _ = app.update(key(
            "/dev/input/event0",
            monitor::KeyEvent::Pressed(K::KEY_A.0),
        ));
        let _ = app.update(key(
            "/dev/input/event0",
            monitor::KeyEvent::Pressed(K::KEY_RIGHTCTRL.0),
        ));
        assert_eq!(
            app.last.as_ref().map(|last| last.code),
            Some("ControlRight")
        );
        assert!(app.is_pressed(K::KEY_A.0));
        assert_eq!(app.held_mods(), [true, false, false, false]);

        let _ = app.update(key(
            "/dev/input/event1",
            monitor::KeyEvent::Released(K::KEY_A.0),
        ));
        assert!(
            app.is_pressed(K::KEY_A.0),
            "another device's release cannot clear this key"
        );
        let _ = app.update(key(
            "/dev/input/event1",
            monitor::KeyEvent::Pressed(K::KEY_B.0),
        ));
        assert_eq!(
            app.last.as_ref().map(|last| last.code),
            Some("ControlRight")
        );
        let _ = app.update(key(
            "/dev/input/event0",
            monitor::KeyEvent::Released(K::KEY_A.0),
        ));
        assert!(!app.is_pressed(K::KEY_A.0));
    }

    #[test]
    fn tester_filter_switches_keep_held_state_and_clear_last_key() {
        use evdev::KeyCode as K;
        let mut app = app();
        let first = PathBuf::from("/dev/input/event0");
        let second = PathBuf::from("/dev/input/event1");
        app.phys_press(&first, K::KEY_A.0);
        let _ = app.update(Message::SetView(View::Tester));
        assert!(
            app.last.is_none(),
            "entering Tester clears an unfiltered preview"
        );
        app.phys_press(&second, K::KEY_LEFTSHIFT.0);
        assert!(app.last.is_some(), "All keyboards accepts either device");
        assert!(app.is_pressed(K::KEY_A.0));
        assert_eq!(app.held_mods(), [false, true, false, false]);

        let _ = app.update(Message::SelectDevice(first.to_string_lossy().into_owned()));
        assert!(app.last.is_none());
        assert!(app.is_pressed(K::KEY_A.0));
        assert_eq!(app.held_mods(), [false; 4]);
        app.phys_press(&first, K::KEY_B.0);
        let _ = app.update(Message::SelectDevice(second.to_string_lossy().into_owned()));
        assert!(app.last.is_none());
        assert!(!app.is_pressed(K::KEY_A.0));
        assert_eq!(app.held_mods(), [false, true, false, false]);

        let _ = app.update(Message::SelectDevice("all".to_owned()));
        assert!(app.is_pressed(K::KEY_A.0));
        assert_eq!(app.held_mods(), [false, true, false, false]);
        let _ = app.update(Message::SelectDevice(first.to_string_lossy().into_owned()));
        let _ = app.update(Message::SetView(View::Keyboard));
        assert_eq!(
            app.held_mods(),
            [false, true, false, false],
            "the editor still observes all keyboards"
        );
    }

    #[test]
    fn tester_filter_follows_replugged_keyboard() {
        use evdev::KeyCode as K;
        // A service restart may reuse the same path; a physical replug may
        // assign a new one. Both must restore testing for the selected device.
        for path in ["/dev/input/event0", "/dev/input/event7"] {
            let mut app = app();
            let _ = app.update(started(keyboard::FORM_FULL, true));
            let _ = app.update(Message::SetView(View::Tester));
            let _ = app.update(Message::SelectDevice("/dev/input/event0".to_owned()));
            app.phys_press(&PathBuf::from("/dev/input/event0"), K::KEY_LEFTSHIFT.0);
            let _ = app.update(Message::Monitor(monitor::Event::Disconnected(
                PathBuf::from("/dev/input/event0"),
            )));
            assert_eq!(app.held_mods(), [false; 4]);
            let _ = app.update(connected(path, "Test Keyboard", keyboard::FORM_FULL, true));
            assert_eq!(app.devices.len(), 1);
            assert!(app.devices[0].connected);
            assert_eq!(app.device, path);
            let _ = app.update(Message::Monitor(monitor::Event::Key {
                device: PathBuf::from(path),
                event: monitor::KeyEvent::Pressed(K::KEY_A.0),
            }));
            assert!(app.is_pressed(K::KEY_A.0));
            assert_eq!(app.last.as_ref().map(|last| last.code), Some("KeyA"));
            let _ = app.update(Message::Monitor(monitor::Event::Key {
                device: PathBuf::from(path),
                event: monitor::KeyEvent::Released(K::KEY_A.0),
            }));
            assert!(!app.is_pressed(K::KEY_A.0));
        }
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

    /// A keyboard the remapper holds, plus the keyboard it re-emits on.
    fn remapped_pair() -> Message {
        let mut physical = test_device(
            "/dev/input/event5",
            "Test Keyboard",
            keyboard::FORM_TKL,
            true,
        );
        physical.name = "@HFD NEO80".to_owned();
        let mut output = test_device(
            "/dev/input/event19",
            "xremap pid=42",
            keyboard::FORM_TKL,
            false,
        );
        output.name = "xremap pid=42".to_owned();
        output.virtual_device = true;
        Message::Monitor(monitor::Event::Started(vec![physical, output]))
    }

    fn grabbed(paths: &[&str]) -> Message {
        Message::Monitor(monitor::Event::Grabbed(
            paths.iter().map(PathBuf::from).collect(),
        ))
    }

    #[test]
    fn a_grabbed_keyboard_is_named_in_the_tester_and_the_picker() {
        let mut app = app();
        let _ = app.update(remapped_pair());
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::SelectDevice("/dev/input/event5".to_owned()));
        assert!(
            app.grabbed_selection().is_none(),
            "nothing is held before the monitor reports a grab"
        );

        let _ = app.update(grabbed(&["/dev/input/event5"]));
        assert_eq!(
            app.grabbed_selection().map(|device| device.name.as_str()),
            Some("@HFD NEO80")
        );
        // The picker points at the keyboard the keys reappear on; which
        // keyboards are held is the tester's story to tell.
        let subs: Vec<String> = app
            .device_entries()
            .into_iter()
            .map(|(_, _, sub)| sub)
            .collect();
        assert!(subs.iter().any(|sub| sub.ends_with("· remapped output")));
        assert!(!subs.iter().any(|sub| sub.contains("held by remapping")));

        // Selecting a keyboard the remapper leaves alone is unaffected.
        let _ = app.update(Message::SelectDevice("/dev/input/event19".to_owned()));
        assert!(app.grabbed_selection().is_none());
        let _ = app.update(Message::SelectDevice("all".to_owned()));
        assert!(
            app.grabbed_selection().is_none(),
            "All keyboards still hears every readable keyboard"
        );

        // Releasing the keyboards clears the notice.
        let _ = app.update(Message::SelectDevice("/dev/input/event5".to_owned()));
        let _ = app.update(grabbed(&[]));
        assert!(app.grabbed_selection().is_none());
    }

    #[test]
    fn the_status_chip_offers_the_step_that_makes_sense() {
        let mut app = app();
        assert_eq!(
            app.remapping_toggle(),
            None,
            "an unknown service state controls nothing"
        );
        for (status, expected) in [
            (service::Status::Active, Some(service::Remapping::Off)),
            (service::Status::Inactive, Some(service::Remapping::On)),
            (service::Status::Failed, Some(service::Remapping::On)),
            (service::Status::NotFound, None),
            (service::Status::Unavailable, None),
        ] {
            let _ = app.update(Message::ServiceStatus(status));
            assert_eq!(app.remapping_toggle(), expected, "{status:?}");
        }

        let _ = app.update(Message::ServiceStatus(service::Status::Active));
        let _ = app.update(Message::SetRemapping(service::Remapping::Off));
        assert_eq!(
            app.remapping_toggle(),
            None,
            "the stop is still on its way to the service"
        );
        let _ = app.update(Message::RemappingSwitched {
            target: service::Remapping::Off,
            result: Ok(()),
        });
        assert_eq!(
            app.remapping_toggle(),
            Some(service::Remapping::On),
            "a stopped service reads the same however it was stopped"
        );

        // A restart in flight would fight the request.
        app.applying = Some(app.apply_seq);
        assert_eq!(app.remapping_toggle(), None);
    }

    #[test]
    fn the_chip_starts_a_service_keyloom_did_not_stop_itself() {
        // Reopening after a pause finds the unit stopped, with nothing
        // in this session's state saying Keyloom is what stopped it.
        for status in [service::Status::Inactive, service::Status::Failed] {
            let mut app = app();
            let _ = app.update(Message::ServiceStatus(status));
            assert_eq!(
                app.remapping_toggle(),
                Some(service::Remapping::On),
                "{status:?}"
            );

            let _ = app.update(Message::SetRemapping(service::Remapping::On));
            assert_eq!(
                app.switching,
                Some(service::Remapping::On),
                "{status:?}: pressing the chip must actually start the service"
            );
        }
    }

    #[test]
    fn a_mapping_change_never_starts_a_service_that_is_not_running() {
        for status in [service::Status::Inactive, service::Status::Failed] {
            let mut app = app();
            let _ = app.update(Message::ServiceStatus(status));
            let _ = app.update(Message::SelectProfile("mac".to_owned()));
            assert_eq!(
                app.apply_step(app.apply_seq),
                ApplyStep::Skip,
                "{status:?}: only the chip starts remapping"
            );
        }
    }

    #[test]
    fn only_the_chip_puts_remapping_back() {
        let mut app = app();
        let _ = app.update(Message::ServiceStatus(service::Status::Active));
        let _ = app.update(Message::SetView(View::Tester));

        let _ = app.update(Message::SetRemapping(service::Remapping::Off));
        assert_eq!(
            app.switching,
            Some(service::Remapping::Off),
            "the stop is still on its way"
        );
        let _ = app.update(Message::RemappingSwitched {
            target: service::Remapping::Off,
            result: Ok(()),
        });
        assert_eq!(app.service, Some(service::Status::Inactive));
        assert_eq!(app.switching, None);

        // Nothing else starts remapping again: not leaving the tester,
        // not moving between views, not closing the window.
        for view in [View::Keyboard, View::Shortcuts, View::Tester] {
            let _ = app.update(Message::SetView(view));
            assert_eq!(
                app.service,
                Some(service::Status::Inactive),
                "leaving for {view:?} must not start remapping"
            );
            assert_eq!(app.switching, None);
        }
        assert!(app.on_app_exit().is_none(), "nothing delays the close");
        assert_eq!(app.service, Some(service::Status::Inactive));

        // Pressing the chip is the only way back.
        let _ = app.update(Message::SetRemapping(service::Remapping::On));
        assert_eq!(app.switching, Some(service::Remapping::On));
        let _ = app.update(Message::RemappingSwitched {
            target: service::Remapping::On,
            result: Ok(()),
        });
        assert_eq!(app.service, Some(service::Status::Active));
        assert!(app.toast.is_none(), "a clean stop and start says nothing");
    }

    #[test]
    fn a_refused_switch_reports_the_state_the_service_is_left_in() {
        let mut app = app();
        let _ = app.update(Message::ServiceStatus(service::Status::Active));
        let _ = app.update(Message::SetView(View::Tester));

        let _ = app.update(Message::SetRemapping(service::Remapping::Off));
        let _ = app.update(Message::RemappingSwitched {
            target: service::Remapping::Off,
            result: Err(service::Error::Refused("unit not loaded".to_owned())),
        });
        assert_eq!(
            app.service,
            Some(service::Status::Active),
            "a refused stop leaves remapping running"
        );
        assert_eq!(app.switching, None);
        let toast = app.toast.as_ref().expect("the failure is reported");
        assert_eq!(toast.text, "Could not pause remapping");
        assert_eq!(toast.sub, "unit not loaded");
        assert_eq!(
            app.remapping_toggle(),
            Some(service::Remapping::Off),
            "the chip still offers the stop it could not make"
        );

        // A refused start leaves remapping stopped, and offers a retry.
        let _ = app.update(Message::SetRemapping(service::Remapping::Off));
        let _ = app.update(Message::RemappingSwitched {
            target: service::Remapping::Off,
            result: Ok(()),
        });
        let _ = app.update(Message::SetRemapping(service::Remapping::On));
        let _ = app.update(Message::RemappingSwitched {
            target: service::Remapping::On,
            result: Err(service::Error::Refused("job failed".to_owned())),
        });
        assert_eq!(
            app.toast.as_ref().map(|toast| toast.text.as_str()),
            Some("Could not resume remapping")
        );
        assert_eq!(
            app.remapping_toggle(),
            Some(service::Remapping::On),
            "the chip still offers the start it could not make"
        );
    }

    #[test]
    fn changes_made_while_remapping_is_off_never_restart_it() {
        let mut app = app();
        let _ = app.update(Message::ServiceStatus(service::Status::Active));
        let _ = app.update(Message::SetRemapping(service::Remapping::Off));
        let _ = app.update(Message::RemappingSwitched {
            target: service::Remapping::Off,
            result: Ok(()),
        });

        // A profile switch still rewrites the config, but restarting
        // would both turn remapping back on and be pointless: the
        // service reads the file when the chip starts it.
        let _ = app.update(Message::SelectProfile("mac".to_owned()));
        let seq = app.apply_seq;
        assert_eq!(app.apply_step(seq), ApplyStep::Skip);
        let _ = app.update(Message::Apply(seq));
        assert!(!app.apply_in_progress());
        assert!(app.applying.is_none(), "no restart went out");

        // Changes made once remapping is back apply as usual.
        let _ = app.update(Message::SetRemapping(service::Remapping::On));
        let _ = app.update(Message::RemappingSwitched {
            target: service::Remapping::On,
            result: Ok(()),
        });
        let _ = app.update(Message::SelectProfile("laptop".to_owned()));
        assert_eq!(app.apply_step(app.apply_seq), ApplyStep::Restart);
    }
}

//! Application state and update logic for the Keyloom GUI.
//!
//! The interface follows the design export in `design/`. Profiles with
//! their mappings and layers persist via cosmic-config, and every
//! change regenerates the xremap configuration written to the user's
//! config directory and restarts the xremap user service (debounced)
//! so it takes effect; shortcut groups are still previewed in memory
//! only.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, ConfigSet, CosmicConfigEntry};
use cosmic::iced::Subscription;
use cosmic::iced::futures::{Stream, StreamExt};
use cosmic::prelude::*;

use crate::apps;
use crate::config::{
    self, KeyboardLayouts, KeyloomConfig, LayoutOverride, ProfileState, SetupState,
};
use crate::keyboard;
use crate::monitor;
use crate::service;
use crate::setup;
use crate::ui;
use crate::ui::model::{
    self, AppRef, AppScope, Chord, Group, Layer, LayerKey, Mapping, Maps, Profile, Rule,
    key_by_evdev, key_name,
};
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
    /// The application scope chooser of one shortcut group.
    GroupScope(usize),
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
    /// Layers, with the mappings a layer change can touch (a key that
    /// takes on holding a layer gives up its hold action).
    Layers {
        layers: HashMap<String, Vec<Layer>>,
        maps: HashMap<String, Maps>,
    },
    /// Application scopes, with the mappings and shortcut groups they
    /// hold.
    Apps {
        apps: HashMap<String, Vec<AppScope>>,
        maps: HashMap<String, Maps>,
        groups: HashMap<String, Vec<Group>>,
    },
    /// A deleted profile: its list position, mappings, layers,
    /// application scopes, and groups.
    Profile {
        index: usize,
        profile: Profile,
        maps: Maps,
        layers: Vec<Layer>,
        apps: Vec<AppScope>,
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

/// What the application picker is choosing applications for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerTarget {
    /// A new application scope, shown on the deck once made.
    NewScope,
    /// The applications of an existing scope.
    Scope(String),
    /// A new application scope that limits a shortcut group.
    Group(usize),
}

/// The application picker dialog while it is open.
#[derive(Clone, Debug)]
pub struct Picker {
    pub target: PickerTarget,
    pub query: String,
    /// Applications chosen so far.
    pub chosen: Vec<AppRef>,
    /// A name typed for an application the lists do not offer.
    pub custom: String,
    /// What the desktop knows, once loaded.
    pub catalog: Option<apps::Catalog>,
}

impl Picker {
    /// Whether an application is among the chosen ones.
    pub fn has(&self, id: &str) -> bool {
        self.chosen.iter().any(|app| app.id == id)
    }
}

/// A key's mapping in the shown scope: its own, or one inherited from
/// a more general scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effective<'a> {
    pub mapping: &'a Mapping,
    /// Whether the mapping's scope is exactly the shown one.
    pub own: bool,
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
    /// Whether the technical details (paths, the unit, the commands)
    /// are shown; off until asked for.
    pub details: bool,
}

impl Setup {
    fn new() -> Self {
        Self {
            page: SetupPage::Welcome,
            facts: None,
            probing: true,
            busy: None,
            error: None,
            details: false,
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
    /// Show and edit one of the profile's layers on the deck, or
    /// (`None`) the normal keys.
    SetLayer(Option<String>),
    ToggleLayers,
    /// Add a layer and start choosing the key that holds it.
    AddLayer,
    /// Choose the key that holds the active layer: the next key
    /// clicked on the deck or pressed on a keyboard.
    ChooseLayerKey,
    /// Stop choosing a layer key; a new layer that never got one is
    /// dropped again.
    CancelLayerKey,
    /// Ask for confirmation before deleting the active layer.
    DeleteLayer,
    DeleteLayerConfirm,
    DeleteLayerCancel,
    /// Start (or cancel) renaming the active layer.
    RenameLayerToggle,
    RenameLayerInput(String),
    RenameLayerCommit,
    /// Show and edit one of the profile's application scopes on the
    /// deck, or (`None`) every application.
    SetAppScope(Option<String>),
    ToggleApps,
    /// Open the application picker for a new scope.
    AddAppScope,
    /// Open the application picker for the shown scope's applications.
    ChangeApps,
    /// Open the application picker for a new scope that limits a
    /// shortcut group.
    GroupAppScope(usize),
    /// Limit a shortcut group to an application scope (empty: none).
    SetGroupScope {
        group: usize,
        scope: String,
    },
    PickerQuery(String),
    /// Choose, or unchoose, an application in the picker.
    PickerToggle(AppRef),
    PickerCustom(String),
    /// Choose the application typed into the picker by name.
    PickerAddCustom,
    PickerCancel,
    PickerConfirm,
    /// The desktop's applications, for the picker.
    AppsCatalog(apps::Catalog),
    /// Ask for confirmation before deleting the shown application
    /// scope with its remaps.
    DeleteAppScope,
    DeleteAppScopeConfirm,
    DeleteAppScopeCancel,
    /// Start (or cancel) renaming the shown application scope.
    RenameAppToggle,
    RenameAppInput(String),
    RenameAppCommit,
    /// Keep the selected key as it is in the shown scope, instead of
    /// the remap it inherits from a more general one.
    NormalKeyHere,
    /// Open a mapping from the remaps list in the scope it belongs to.
    EditMapping(usize),
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
    /// Ask for confirmation before removing a mapping (by its position
    /// in the active profile's list) from the remaps list.
    RemoveMapping(usize),
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
    /// Show or hide the technical details on setup's step pages.
    SetupToggleDetails,
    /// Open a web page in the user's browser.
    OpenUrl(&'static str),
    /// Whether the browser could be asked to open the page.
    UrlOpened {
        url: &'static str,
        opened: bool,
    },
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
    /// Position of the mapping awaiting removal confirmation in the
    /// active profile's list.
    pub confirm_remove_mapping: Option<usize>,
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
    pub profile_layers: HashMap<String, Vec<Layer>>,
    pub profile_apps: HashMap<String, Vec<AppScope>>,
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
    /// The layer shown and edited on the deck (`None`: the normal keys).
    pub layer: Option<String>,
    pub layers_open: bool,
    /// The next key clicked on the deck or pressed on a keyboard
    /// becomes the active layer's key.
    pub choosing_layer_key: bool,
    /// In-progress rename of the active layer (the edited text).
    pub rename_layer: Option<String>,
    /// Layer id awaiting delete confirmation in the modal dialog.
    pub confirm_delete_layer: Option<String>,
    /// The application scope shown and edited on the deck (`None`:
    /// every application).
    pub app_scope: Option<String>,
    pub apps_open: bool,
    /// In-progress rename of the shown application scope.
    pub rename_app: Option<String>,
    /// Application scope id awaiting delete confirmation.
    pub confirm_delete_app: Option<String>,
    /// The application picker, while it is open.
    pub picker: Option<Picker>,
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

    /// The application scope shown on the deck, as mappings store it:
    /// empty for every application.
    pub fn app_id(&self) -> &str {
        self.app_scope.as_deref().unwrap_or("")
    }

    /// Application scopes of the active profile.
    pub fn app_scopes(&self) -> &[AppScope] {
        self.profile_apps
            .get(&self.profile)
            .map_or(&[], Vec::as_slice)
    }

    /// The application scope shown on the deck, if one is.
    pub fn active_app_scope(&self) -> Option<&AppScope> {
        let id = self.app_scope.as_deref()?;
        self.app_scopes().iter().find(|scope| scope.id == id)
    }

    /// Name of an application scope; every application for none.
    pub fn app_scope_name(&self, id: &str) -> String {
        self.app_scopes()
            .iter()
            .find(|scope| scope.id == id)
            .map_or_else(|| "All applications".to_owned(), |scope| scope.name.clone())
    }

    /// The scope, if any, that already covers an application in the
    /// active profile.
    pub fn scope_of_app(&self, id: &str) -> Option<&AppScope> {
        self.app_scopes().iter().find(|scope| scope.has_app(id))
    }

    /// The active profile's mapping for one key in the shown scope:
    /// its own, or the most specific one it inherits (see
    /// [`model::scope_rank`]).
    pub fn effective_mapping(&self, code: &str) -> Option<Effective<'_>> {
        let (device, app) = (self.device.as_str(), self.app_id());
        self.maps()
            .iter()
            .filter(|(key, mapping)| key == code && mapping.applies_in(device, app))
            .min_by_key(|(_, mapping)| mapping.rank())
            .map(|(_, mapping)| Effective {
                mapping,
                own: mapping.scoped_to(device, app),
            })
    }

    /// The mapping that applies to a key in the shown scope.
    pub fn mapping(&self, code: &str) -> Option<&Mapping> {
        self.effective_mapping(code)
            .map(|effective| effective.mapping)
    }

    /// The key's own mapping in exactly the shown scope.
    pub fn own_mapping(&self, code: &str) -> Option<&Mapping> {
        self.effective_mapping(code)
            .filter(|effective| effective.own)
            .map(|effective| effective.mapping)
    }

    /// How many of a key's mappings belong to scopes other than the
    /// one applying here: the deck marks such keys.
    pub fn other_scopes(&self, code: &str) -> usize {
        let total = self.maps().iter().filter(|(key, _)| key == code).count();
        total - usize::from(self.effective_mapping(code).is_some())
    }

    /// Whether the deck shows a specific scope (an application scope,
    /// or one keyboard) rather than every keyboard in every
    /// application.
    pub fn in_specific_scope(&self) -> bool {
        self.app_scope.is_some() || !model::every_device(&self.device)
    }

    /// Where a mapping inherited in the shown scope comes from.
    pub fn inherited_from(&self, mapping: &Mapping) -> &'static str {
        if mapping.app != self.app_id() {
            "all applications"
        } else {
            "all keyboards"
        }
    }

    /// The shown scope as a place: `in COSMIC Terminal`, `on Keychron
    /// K2 Pro`, or `everywhere`.
    pub fn here_label(&self) -> String {
        if let Some(scope) = self.active_app_scope() {
            format!("in {}", scope.name)
        } else if model::every_device(&self.device) {
            "everywhere".to_owned()
        } else {
            format!("on {}", self.device_label(&self.device))
        }
    }

    /// The shown scope for toasts: `all keyboards · in COSMIC Terminal`.
    pub fn scope_summary(&self) -> String {
        let mut summary = self.device_label(&self.device).to_lowercase();
        if let Some(scope) = self.active_app_scope() {
            summary.push_str(&format!(" · in {}", scope.name));
        }
        summary
    }

    /// A mapping's scope for lists: `All keyboards · COSMIC Terminal`.
    pub fn scope_label(&self, mapping: &Mapping) -> String {
        let mut label = self.device_label(&mapping.device);
        if !mapping.app.is_empty() {
            label.push_str(&format!(" · {}", self.app_scope_name(&mapping.app)));
        }
        label
    }

    /// How many mappings and shortcut rules an application scope holds.
    pub fn scope_count(&self, id: &str) -> usize {
        let mappings = self
            .maps()
            .iter()
            .filter(|(_, mapping)| mapping.app == id)
            .count();
        let rules: usize = self
            .groups()
            .iter()
            .filter(|group| group.scope == id)
            .map(|group| group.rules.len())
            .sum();
        mappings + rules
    }

    /// Shortcut groups of the active profile.
    pub fn groups(&self) -> &[Group] {
        self.profile_groups
            .get(&self.profile)
            .map_or(&[], Vec::as_slice)
    }

    /// What the generator reads from the active profile.
    pub fn rules(&self) -> xremap::Rules<'_> {
        xremap::Rules {
            maps: self.maps(),
            layers: self.layers(),
            apps: self.app_scopes(),
            groups: self.groups(),
        }
    }

    /// Layers of the active profile.
    pub fn layers(&self) -> &[Layer] {
        self.profile_layers
            .get(&self.profile)
            .map_or(&[], Vec::as_slice)
    }

    /// The layer shown on the deck, if one is.
    pub fn active_layer(&self) -> Option<&Layer> {
        let id = self.layer.as_deref()?;
        self.layers().iter().find(|layer| layer.id == id)
    }

    /// The layer a key holds in the active profile, if any.
    pub fn layer_held_by(&self, code: &str) -> Option<&Layer> {
        self.layers().iter().find(|layer| layer.trigger == code)
    }

    /// Why a key cannot take a job in the active layer, as a toast.
    fn layer_job_blocker(&self, code: &str) -> Option<(String, String)> {
        let layer = self.active_layer()?;
        let name = key_name(code);
        if layer.trigger == code {
            return Some((
                format!("{name} holds this layer"),
                "The key that holds a layer cannot take a job in it. Use “Change key” to hold the layer with another key.".to_owned(),
            ));
        }
        if xremap::is_modifier_key(code) {
            return Some((
                format!("{name} keeps its job"),
                "Shift, Control, Alt and Super work the same in every layer.".to_owned(),
            ));
        }
        if let Some(other) = self.layer_held_by(code) {
            return Some((
                format!("{name} holds the {} layer", other.name),
                "A key that holds a layer cannot take a job in another one.".to_owned(),
            ));
        }
        None
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
    /// name and apply in the shown scope, as `(group index, rule index,
    /// rule)`.
    pub fn combos_for(&self, key: &str) -> Vec<(usize, usize, &Rule)> {
        let app = self.app_id();
        self.groups()
            .iter()
            .enumerate()
            .filter(|(_, group)| group.scope.is_empty() || group.scope == app)
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
                &self.profile_state(),
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
        let yaml = xremap::generate(self.rules(), |id| self.device_label(id));
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
        let name = key_name(code);
        // Holding a layer key activates its layer, so a hold action
        // would never run.
        if self.mode == Mode::Hold
            && let Some(layer) = self.layer_held_by(code)
        {
            let layer = layer.name.clone();
            self.flash(
                format!("{name} holds the {layer} layer"),
                "Delete the layer to give this key a hold action instead.",
            );
            return;
        }
        // Reject a no-op self-mapping. Assigning the key's own name is
        // only meaningful when holding the key does something else
        // (a hold action, or a layer).
        if action == name {
            let other = self.mapping(code).and_then(|mapping| {
                if self.mode == Mode::Hold {
                    mapping.tap.clone()
                } else {
                    mapping.hold.clone()
                }
            });
            let holds_layer = self.mode == Mode::Tap && self.layer_held_by(code).is_some();
            if (other.is_none() && !holds_layer) || other.as_deref() == Some(action) {
                self.flash(
                    format!("{name} already does that"),
                    "Mapping a key to itself would change nothing — choose a different output.",
                );
                return;
            }
        }
        self.undo = Some(Undo::Maps(self.profile_maps.clone()));
        let hold = self.mode == Mode::Hold;
        let entry = self.scoped_entry(code);
        if hold {
            entry.hold = Some(action.to_owned());
        } else {
            entry.tap = Some(action.to_owned());
        }
        entry.normal = false;

        let held = if hold { " held" } else { "" };
        let scope = self.scope_summary();
        self.flash(
            format!("{}{held} → {action}", key_name(code)),
            format!("applies automatically · {scope}"),
        );
        self.persist();
    }

    /// The shown scope's own mapping for a key, made on demand as a
    /// copy of the mapping the key inherits there, so that a change in
    /// one application (or on one keyboard) keeps the rest of the
    /// key's behavior.
    fn scoped_entry(&mut self, code: &str) -> &mut Mapping {
        let device = self.device.clone();
        let app = self.app_id().to_owned();
        let inherited = self
            .effective_mapping(code)
            .filter(|effective| !effective.own)
            .map(|effective| effective.mapping.clone());
        let maps = self.profile_maps.entry(self.profile.clone()).or_default();
        let index = match maps
            .iter()
            .position(|(key, mapping)| key == code && mapping.scoped_to(&device, &app))
        {
            Some(index) => index,
            None => {
                let mut mapping = inherited.unwrap_or_default();
                mapping.device = device;
                mapping.app = app;
                maps.push((code.to_owned(), mapping));
                maps.len() - 1
            }
        };
        &mut maps[index].1
    }

    /// Keep the selected key as it is in the shown scope, standing in
    /// for the remap it inherits there.
    fn set_normal_here(&mut self) {
        if self.view != View::Keyboard || self.layer.is_some() || !self.in_specific_scope() {
            return;
        }
        let Some(code) = self.selected else {
            return;
        };
        let name = key_name(code);
        if self.own_mapping(code).is_some_and(|mapping| mapping.normal) {
            self.flash(
                format!("{name} is already a normal key here"),
                "Nothing remaps it in this scope.",
            );
            return;
        }
        self.undo = Some(Undo::Maps(self.profile_maps.clone()));
        let entry = self.scoped_entry(code);
        *entry = Mapping {
            device: entry.device.clone(),
            app: entry.app.clone(),
            normal: true,
            ..Mapping::default()
        };
        let scope = self.scope_summary();
        self.flash(
            format!("{name} stays {name}"),
            format!("normal key · {scope} · applies automatically"),
        );
        self.persist();
    }

    /// The profile state as it is saved. A layer still waiting for its
    /// key is not part of it: without a key it can do nothing yet.
    fn profile_state(&self) -> ProfileState {
        ProfileState {
            profiles: self.profiles.clone(),
            maps: self.profile_maps.clone(),
            layers: self.keyed_layers(),
            apps: self.profile_apps.clone(),
            groups: self.profile_groups.clone(),
            active: self.profile.clone(),
            custom_profiles: u32::try_from(self.custom_profiles).unwrap_or(u32::MAX),
        }
    }

    /// Every profile's layers, without any still waiting for a key.
    fn keyed_layers(&self) -> HashMap<String, Vec<Layer>> {
        self.profile_layers
            .iter()
            .map(|(id, layers)| {
                let layers = layers
                    .iter()
                    .filter(|layer| !layer.trigger.is_empty())
                    .cloned()
                    .collect();
                (id.clone(), layers)
            })
            .collect()
    }

    /// Remember the layers and mappings before a layer change, for
    /// Undo. A layer still waiting for its key is not a state worth
    /// returning to.
    fn snapshot_layers(&mut self) {
        self.undo = Some(Undo::Layers {
            layers: self.keyed_layers(),
            maps: self.profile_maps.clone(),
        });
    }

    /// Leave layer editing: stop choosing a key, show the normal keys,
    /// and drop any pending rename or delete.
    fn leave_layers(&mut self) {
        self.cancel_layer_key();
        self.layer = None;
        self.rename_layer = None;
        self.confirm_delete_layer = None;
    }

    /// Show every application again, dropping any pending rename or
    /// delete of the shown scope.
    fn show_every_app(&mut self) {
        self.app_scope = None;
        self.rename_app = None;
        self.confirm_delete_app = None;
    }

    /// Leave application editing altogether, picker included.
    fn leave_apps(&mut self) {
        self.show_every_app();
        self.picker = None;
    }

    /// Remember the application scopes, mappings, and groups before an
    /// application scope change, for Undo.
    fn snapshot_apps(&mut self) {
        self.undo = Some(Undo::Apps {
            apps: self.profile_apps.clone(),
            maps: self.profile_maps.clone(),
            groups: self.profile_groups.clone(),
        });
    }

    /// Open the application picker and ask the desktop what it knows.
    fn open_picker(&mut self, target: PickerTarget) -> Task<Message> {
        if self.view == View::Tester {
            return Task::none();
        }
        let chosen = match &target {
            PickerTarget::Scope(id) => self
                .app_scopes()
                .iter()
                .find(|scope| &scope.id == id)
                .map(|scope| scope.apps.clone())
                .unwrap_or_default(),
            PickerTarget::NewScope | PickerTarget::Group(_) => Vec::new(),
        };
        self.picker = Some(Picker {
            target,
            query: String::new(),
            chosen,
            custom: String::new(),
            catalog: None,
        });
        self.popover = None;
        self.rename_app = None;
        self.close_sheet();
        cosmic::task::future(async { Message::AppsCatalog(apps::catalog().await) })
    }

    /// Choose an application in the picker, or unchoose it. An
    /// application already covered by another scope of the profile
    /// stays there: one scope per application keeps precedence simple.
    fn pick_app(&mut self, app: AppRef) {
        let Some(picker) = &self.picker else {
            return;
        };
        if picker.has(&app.id) {
            if let Some(picker) = &mut self.picker {
                picker.chosen.retain(|chosen| chosen.id != app.id);
            }
            return;
        }
        let editing = match &picker.target {
            PickerTarget::Scope(id) => Some(id.as_str()),
            PickerTarget::NewScope | PickerTarget::Group(_) => None,
        };
        if let Some(other) = self
            .scope_of_app(&app.id)
            .filter(|scope| Some(scope.id.as_str()) != editing)
        {
            let (name, scope) = (app.name.clone(), other.name.clone());
            self.flash(
                format!("{name} already has its own remaps"),
                format!("It belongs to {scope}. Remove it there first."),
            );
            return;
        }
        if let Some(picker) = &mut self.picker {
            picker.chosen.push(app);
        }
    }

    /// Choose the application typed into the picker by name.
    fn pick_custom_app(&mut self) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        let id = std::mem::take(&mut picker.custom).trim().to_owned();
        if id.is_empty() {
            return;
        }
        // The catalog may know the typed id under a friendlier name.
        let app = picker
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.apps.iter().find(|known| known.app.id == id))
            .map_or_else(
                || AppRef {
                    id: id.clone(),
                    name: id.clone(),
                    aliases: Vec::new(),
                },
                |known| known.app.clone(),
            );
        self.pick_app(app);
    }

    /// Apply the picker's choice to its target and close it.
    fn confirm_picker(&mut self) {
        let Some(picker) = self.picker.take() else {
            return;
        };
        if picker.chosen.is_empty() {
            self.picker = Some(picker);
            return;
        }
        match picker.target {
            PickerTarget::NewScope => {
                let id = self.add_app_scope(picker.chosen);
                let name = self.app_scope_name(&id);
                self.leave_layers();
                self.app_scope = Some(id);
                self.apps_open = true;
                self.flash(
                    format!("{name} added"),
                    "Click a key to choose what it does while this app is in front.",
                );
            }
            PickerTarget::Scope(id) => {
                self.snapshot_apps();
                if let Some(scope) = self
                    .profile_apps
                    .entry(self.profile.clone())
                    .or_default()
                    .iter_mut()
                    .find(|scope| scope.id == id)
                {
                    scope.apps = picker.chosen;
                }
                let name = self.app_scope_name(&id);
                self.flash(format!("{name} updated"), "applies automatically");
                self.persist();
            }
            PickerTarget::Group(index) => {
                let id = self.add_app_scope(picker.chosen);
                self.set_group_scope(index, id);
            }
        }
    }

    /// Add an application scope to the active profile, named after
    /// its first application, and return its id.
    fn add_app_scope(&mut self, apps: Vec<AppRef>) -> String {
        self.snapshot_apps();
        // Number past every id ever used here, so ids stay unique.
        let number = self
            .app_scopes()
            .iter()
            .filter_map(|scope| scope.id.strip_prefix("app-")?.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let id = format!("app-{number}");
        let name = match apps.as_slice() {
            [first] => first.name.clone(),
            [first, rest @ ..] => format!("{} + {}", first.name, rest.len()),
            [] => format!("Application {number}"),
        };
        self.profile_apps
            .entry(self.profile.clone())
            .or_default()
            .push(AppScope {
                id: id.clone(),
                name,
                apps,
            });
        self.persist();
        id
    }

    /// Delete an application scope of the active profile with its
    /// mappings and shortcut groups, once confirmed.
    fn delete_app_scope(&mut self, id: &str) {
        if self.view != View::Keyboard {
            return;
        }
        let Some(index) = self.app_scopes().iter().position(|scope| scope.id == id) else {
            return;
        };
        self.snapshot_apps();
        let scope = self
            .profile_apps
            .get_mut(&self.profile)
            .map(|scopes| scopes.remove(index));
        if let Some(maps) = self.profile_maps.get_mut(&self.profile) {
            maps.retain(|(_, mapping)| mapping.app != id);
        }
        if let Some(groups) = self.profile_groups.get_mut(&self.profile) {
            groups.retain(|group| group.scope != id);
        }
        if self.app_scope.as_deref() == Some(id) {
            self.show_every_app();
            self.clear_sheet();
        }
        let name = scope.map(|scope| scope.name).unwrap_or_default();
        self.flash(
            format!("{name} removed"),
            "Its keys work like everywhere else again. Undo restores it.",
        );
        self.persist();
    }

    /// Apply the pending rename to the shown application scope.
    fn commit_app_rename(&mut self) {
        let Some(name) = self.rename_app.take() else {
            return;
        };
        let name = name.trim().to_owned();
        let Some(scope) = self.active_app_scope() else {
            return;
        };
        if name.is_empty() || name == scope.name {
            return;
        }
        let id = scope.id.clone();
        self.snapshot_apps();
        if let Some(scope) = self
            .profile_apps
            .get_mut(&self.profile)
            .and_then(|scopes| scopes.iter_mut().find(|scope| scope.id == id))
        {
            scope.name.clone_from(&name);
        }
        self.flash("Application renamed", format!("now called {name}"));
        self.persist();
    }

    /// Limit a shortcut group to an application scope, or (empty) to
    /// none.
    fn set_group_scope(&mut self, index: usize, scope: String) {
        if !scope.is_empty() && !self.app_scopes().iter().any(|known| known.id == scope) {
            return;
        }
        let mut name = None;
        self.mutate_groups(|groups| {
            if let Some(group) = groups.get_mut(index) {
                group.scope.clone_from(&scope);
                name = Some(group.name.clone());
            }
        });
        self.popover = None;
        if let Some(name) = name {
            let scope = self.app_scope_name(&scope);
            self.flash(format!("{name} · {scope}"), "applies automatically");
        }
    }

    /// Stop choosing a layer key. A new layer that never got its key
    /// is dropped again: without one it can do nothing.
    fn cancel_layer_key(&mut self) {
        if !self.choosing_layer_key {
            return;
        }
        self.choosing_layer_key = false;
        if let Some(id) = self.layer.clone()
            && self
                .active_layer()
                .is_some_and(|layer| layer.trigger.is_empty())
        {
            if let Some(layers) = self.profile_layers.get_mut(&self.profile) {
                layers.retain(|layer| layer.id != id);
            }
            self.layer = None;
            self.toast = None;
        }
    }

    /// Add a layer to the active profile and start choosing its key.
    fn add_layer(&mut self) {
        if self.view != View::Keyboard || self.choosing_layer_key {
            return;
        }
        if self.layers().len() >= xremap::MAX_LAYERS {
            self.flash(
                format!("Up to {} layers per profile", xremap::MAX_LAYERS),
                "Delete a layer to make room for another.",
            );
            return;
        }
        // Number past every id ever used here, so names stay unique.
        let number = self
            .layers()
            .iter()
            .filter_map(|layer| layer.id.strip_prefix("layer-")?.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let id = format!("layer-{number}");
        let name = format!("Layer {number}");
        self.profile_layers
            .entry(self.profile.clone())
            .or_default()
            .push(Layer {
                id: id.clone(),
                name: name.clone(),
                trigger: String::new(),
                keys: Vec::new(),
            });
        self.show_every_app();
        self.layer = Some(id);
        self.layers_open = true;
        self.choosing_layer_key = true;
        self.rename_layer = None;
        self.remaps_open = false;
        self.confirm_remove_mapping = None;
        self.close_sheet();
        self.flash(
            format!("{name} added"),
            "Click the key that will hold it, or press it on your keyboard.",
        );
    }

    /// Make a key hold the active layer. The key gives up any hold
    /// action and any layer jobs it had: held, it activates the layer.
    fn set_layer_trigger(&mut self, code: &str) {
        let Some(id) = self.layer.clone() else {
            self.choosing_layer_key = false;
            return;
        };
        let name = key_name(code);
        if let Some(other) = self
            .layers()
            .iter()
            .find(|layer| layer.id != id && layer.trigger == code)
        {
            let other = other.name.clone();
            self.flash(
                format!("{name} already holds {other}"),
                "Choose a different key for this layer.",
            );
            return;
        }
        self.snapshot_layers();
        let mut hold_removed = false;
        if let Some(maps) = self.profile_maps.get_mut(&self.profile) {
            for (_, mapping) in maps.iter_mut().filter(|(key, _)| key == code) {
                hold_removed |= mapping.hold.take().is_some();
            }
            // A mapping left with nothing to do is dropped.
            maps.retain(|(key, mapping)| key != code || mapping.tap.is_some() || mapping.normal);
        }
        let mut layer_name = String::new();
        if let Some(layers) = self.profile_layers.get_mut(&self.profile) {
            for layer in layers.iter_mut() {
                layer.keys.retain(|key| key.code != code);
            }
            if let Some(layer) = layers.iter_mut().find(|layer| layer.id == id) {
                layer.trigger = code.to_owned();
                layer_name.clone_from(&layer.name);
            }
        }
        self.choosing_layer_key = false;
        self.flash(
            format!("Hold {name} for {layer_name}"),
            if hold_removed {
                "Its hold action gave way to the layer · applies automatically"
            } else {
                "applies automatically"
            },
        );
        self.persist();
    }

    /// Give the selected key a job in the active layer.
    fn set_layer_job(&mut self, code: &str, action: &str) {
        if self.view != View::Keyboard {
            return;
        }
        let Some(id) = self.layer.clone() else {
            return;
        };
        if let Some((text, sub)) = self.layer_job_blocker(code) {
            self.flash(text, sub);
            return;
        }
        // Naming the key itself changes nothing unless a mapping
        // turned the key into something else.
        let name = key_name(code);
        if action == name && self.mapping(code).and_then(|m| m.tap.as_deref()).is_none() {
            self.flash(
                format!("{name} already does that"),
                "Mapping a key to itself would change nothing — choose a different output.",
            );
            return;
        }
        self.snapshot_layers();
        let device = self.device.clone();
        let mut trigger = String::new();
        if let Some(layer) = self
            .profile_layers
            .entry(self.profile.clone())
            .or_default()
            .iter_mut()
            .find(|layer| layer.id == id)
        {
            trigger.clone_from(&layer.trigger);
            if let Some(job) = layer.keys.iter_mut().find(|key| key.code == code) {
                job.action = action.to_owned();
                job.device.clone_from(&device);
            } else {
                layer.keys.push(LayerKey {
                    code: code.to_owned(),
                    action: action.to_owned(),
                    device: device.clone(),
                    app: String::new(),
                });
            }
        }
        let device = self.device_label(&device).to_lowercase();
        self.flash(
            format!("{} + {name} → {action}", key_name(&trigger)),
            format!("applies automatically · {device}"),
        );
        self.persist();
    }

    /// Take the selected key's job away in the active layer.
    fn remove_layer_job(&mut self, code: &str) {
        let Some(layer) = self.active_layer() else {
            return;
        };
        let name = key_name(code);
        let layer_name = layer.name.clone();
        if layer.key(code).is_none() {
            self.flash(
                format!("{name} already works normally in {layer_name}"),
                "It has no job in this layer.",
            );
            return;
        }
        let id = layer.id.clone();
        self.snapshot_layers();
        if let Some(layers) = self.profile_layers.get_mut(&self.profile)
            && let Some(layer) = layers.iter_mut().find(|layer| layer.id == id)
        {
            layer.keys.retain(|key| key.code != code);
        }
        self.flash(
            format!("{name} back to normal in {layer_name}"),
            "applies automatically",
        );
        self.persist();
    }

    /// Delete a layer of the active profile, once confirmed.
    fn delete_layer(&mut self, id: &str) {
        if self.view != View::Keyboard {
            return;
        }
        let Some(index) = self.layers().iter().position(|layer| layer.id == id) else {
            return;
        };
        self.snapshot_layers();
        let layer = self
            .profile_layers
            .get_mut(&self.profile)
            .map(|layers| layers.remove(index));
        if self.layer.as_deref() == Some(id) {
            self.layer = None;
            self.choosing_layer_key = false;
            self.rename_layer = None;
            self.clear_sheet();
        }
        let name = layer.map(|layer| layer.name).unwrap_or_default();
        self.flash(format!("{name} deleted"), "Undo restores it.");
        self.persist();
    }

    /// Apply the pending rename to the active layer.
    fn commit_layer_rename(&mut self) {
        let Some(name) = self.rename_layer.take() else {
            return;
        };
        let name = name.trim().to_owned();
        let Some(layer) = self.active_layer() else {
            return;
        };
        if name.is_empty() || name == layer.name {
            return;
        }
        let id = layer.id.clone();
        self.snapshot_layers();
        if let Some(layers) = self.profile_layers.get_mut(&self.profile)
            && let Some(layer) = layers.iter_mut().find(|layer| layer.id == id)
        {
            layer.name.clone_from(&name);
        }
        self.flash("Layer renamed", format!("now called {name}"));
        self.persist();
    }

    /// Snapshot, then mutate the active profile's shortcut groups, and
    /// save the result.
    fn mutate_groups(&mut self, mutate: impl FnOnce(&mut Vec<Group>)) {
        if self.view == View::Tester {
            return;
        }
        self.undo = Some(Undo::Groups(self.profile_groups.clone()));
        mutate(self.profile_groups.entry(self.profile.clone()).or_default());
        self.persist();
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
        // Combos land in a "From the keyboard" group of the shown
        // application scope (or of every application).
        let app = self.app_id().to_owned();
        let group_id = if app.is_empty() {
            "kb".to_owned()
        } else {
            format!("kb:{app}")
        };
        self.mutate_groups(|groups| {
            let index = groups.iter().position(|group| group.id == group_id);
            let index = index.unwrap_or_else(|| {
                groups.insert(
                    0,
                    Group {
                        id: group_id.clone(),
                        name: "From the keyboard".to_owned(),
                        scope: app.clone(),
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
        let scope = self
            .active_app_scope()
            .map_or_else(|| "all applications".to_owned(), |scope| scope.name.clone());
        self.flash(
            format!("{}+{key} → {output}", from_mods.join("+")),
            format!("applies automatically · {scope}"),
        );
    }

    /// Restore the selected key — in the active layer while one is
    /// shown, otherwise in the shown scope: its own mapping there goes,
    /// and whatever a more general scope says applies again.
    fn clear_mapping(&mut self) {
        if self.view == View::Tester {
            return;
        }
        let Some(code) = self.selected else {
            return;
        };
        if self.layer.is_some() {
            self.remove_layer_job(code);
            return;
        }
        let (device, app) = (self.device.clone(), self.app_id().to_owned());
        let own = self
            .maps()
            .iter()
            .position(|(key, mapping)| key == code && mapping.scoped_to(&device, &app));
        match own {
            Some(index) => self.remove_mapping_at(index),
            None => {
                let name = key_name(code);
                let sub = match self.mapping(code) {
                    Some(inherited) => {
                        format!("It follows {} here.", self.inherited_from(inherited))
                    }
                    None => "Nothing remaps it.".to_owned(),
                };
                self.flash(format!("{name} has no remap of its own here"), sub);
            }
        }
    }

    /// Remove one mapping, by its position in the active profile's
    /// list.
    fn remove_mapping_at(&mut self, index: usize) {
        let Some((code, mapping)) = self.maps().get(index).cloned() else {
            return;
        };
        self.undo = Some(Undo::Maps(self.profile_maps.clone()));
        if let Some(maps) = self.profile_maps.get_mut(&self.profile) {
            maps.remove(index);
        }
        let name = key_name(&code);
        let text = if mapping.is_general() {
            format!("{name} back to default")
        } else if mapping.app.is_empty() {
            format!("{name} same as all keyboards")
        } else {
            format!("{name} same as all applications")
        };
        self.flash(text, "applies automatically");
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
        let layers = if duplicate {
            self.layers().to_vec()
        } else {
            Vec::new()
        };
        let apps = if duplicate {
            self.app_scopes().to_vec()
        } else {
            Vec::new()
        };
        self.leave_layers();
        self.leave_apps();
        self.profiles.push(Profile {
            id: id.clone(),
            name: name.clone(),
        });
        self.profile_maps.insert(id.clone(), maps);
        self.profile_groups.insert(id.clone(), groups);
        self.profile_layers.insert(id.clone(), layers);
        self.profile_apps.insert(id.clone(), apps);
        self.profile = id;
        self.confirm_remove_mapping = None;
        self.confirm_reset_mappings = None;
        self.popover = None;
        self.rename = None;
        self.clear_sheet();
        self.undo = None;
        let sub = if duplicate {
            "A separate copy of your mappings, layers, applications and shortcuts."
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
        let layers = self.profile_layers.remove(id).unwrap_or_default();
        let apps = self.profile_apps.remove(id).unwrap_or_default();
        let groups = self.profile_groups.remove(id).unwrap_or_default();
        let name = profile.name.clone();
        self.undo = Some(Undo::Profile {
            index,
            profile,
            maps,
            layers,
            apps,
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

        // Choosing the key that holds a layer.
        if !escape && self.view == View::Keyboard && self.choosing_layer_key {
            let Some(cap) = key_by_evdev(scancode) else {
                return;
            };
            self.pressed.insert((device.clone(), scancode));
            self.set_layer_trigger(cap.code);
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
            if self.layer.is_some() {
                self.set_layer_job(selected, &action);
            } else if self.mode == Mode::Combo {
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

        let state = match stored {
            Some(stored) => stored.into_state(),
            None => {
                // Fresh install: an empty Default profile plus the
                // editable starter profiles. Once persisted they are
                // ordinary profiles like any the user creates.
                let default = Profile {
                    id: "default".to_owned(),
                    name: "Default".to_owned(),
                };
                let mut state = ProfileState {
                    maps: HashMap::from([(default.id.clone(), Vec::new())]),
                    layers: HashMap::from([(default.id.clone(), Vec::new())]),
                    active: default.id.clone(),
                    profiles: vec![default],
                    ..ProfileState::default()
                };
                for starter in model::starter_profiles() {
                    state.maps.insert(starter.profile.id.clone(), starter.maps);
                    state
                        .layers
                        .insert(starter.profile.id.clone(), starter.layers);
                    state.profiles.push(starter.profile);
                }
                state
            }
        };
        let ProfileState {
            profiles,
            maps: profile_maps,
            layers: profile_layers,
            apps: profile_apps,
            groups: profile_groups,
            active: profile,
            custom_profiles,
        } = state;
        let custom_profiles = usize::try_from(custom_profiles).unwrap_or(usize::MAX);

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
            profile_layers,
            profile_apps,
            custom_profiles,
            rename: None,
            confirm_delete: None,
            confirm_reset_mappings: None,
            undo: None,
            device: "all".to_owned(),
            form: keyboard::FORM_FULL,
            iso: false,
            keyboard_layouts,
            layer: None,
            layers_open: false,
            choosing_layer_key: false,
            rename_layer: None,
            confirm_delete_layer: None,
            app_scope: None,
            apps_open: false,
            rename_app: None,
            confirm_delete_app: None,
            picker: None,
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
                self.leave_layers();
                self.leave_apps();
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
                self.leave_layers();
                self.leave_apps();
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
                if self.view == View::Keyboard {
                    self.cancel_layer_key();
                    self.layer =
                        layer.filter(|id| self.layers().iter().any(|layer| &layer.id == id));
                    // One context at a time: a layer shows the same
                    // jobs in every application.
                    if self.layer.is_some() {
                        self.show_every_app();
                    }
                    self.rename_layer = None;
                    self.remaps_open = false;
                    self.confirm_remove_mapping = None;
                    self.close_sheet();
                }
            }
            Message::ToggleLayers => self.layers_open = !self.layers_open,
            Message::SetAppScope(scope) => {
                if self.view == View::Keyboard {
                    let scope = scope.filter(|id| self.app_scopes().iter().any(|s| &s.id == id));
                    if scope.is_some() {
                        self.leave_layers();
                    }
                    self.app_scope = scope;
                    self.rename_app = None;
                    self.remaps_open = false;
                    self.confirm_remove_mapping = None;
                    self.close_sheet();
                }
            }
            Message::ToggleApps => self.apps_open = !self.apps_open,
            Message::AddAppScope => {
                if self.view == View::Keyboard {
                    return self.open_picker(PickerTarget::NewScope);
                }
            }
            Message::ChangeApps => {
                if self.view == View::Keyboard
                    && let Some(scope) = self.active_app_scope()
                {
                    let id = scope.id.clone();
                    return self.open_picker(PickerTarget::Scope(id));
                }
            }
            Message::GroupAppScope(index) => {
                if self.view == View::Shortcuts && index < self.groups().len() {
                    return self.open_picker(PickerTarget::Group(index));
                }
            }
            Message::SetGroupScope { group, scope } => {
                if self.view == View::Shortcuts {
                    self.set_group_scope(group, scope);
                }
            }
            Message::PickerQuery(query) => {
                if let Some(picker) = &mut self.picker {
                    picker.query = query;
                }
            }
            Message::PickerToggle(app) => self.pick_app(app),
            Message::PickerCustom(text) => {
                if let Some(picker) = &mut self.picker {
                    picker.custom = text;
                }
            }
            Message::PickerAddCustom => self.pick_custom_app(),
            Message::PickerCancel => self.picker = None,
            Message::PickerConfirm => self.confirm_picker(),
            Message::AppsCatalog(catalog) => {
                if let Some(picker) = &mut self.picker {
                    picker.catalog = Some(catalog);
                }
            }
            Message::DeleteAppScope => {
                if self.view == View::Keyboard
                    && let Some(scope) = self.active_app_scope()
                {
                    self.confirm_delete_app = Some(scope.id.clone());
                }
            }
            Message::DeleteAppScopeConfirm => {
                if let Some(id) = self.confirm_delete_app.take() {
                    self.delete_app_scope(&id);
                }
            }
            Message::DeleteAppScopeCancel => self.confirm_delete_app = None,
            Message::RenameAppToggle => {
                if self.view == View::Keyboard
                    && let Some(scope) = self.active_app_scope()
                {
                    self.rename_app = if self.rename_app.is_some() {
                        None
                    } else {
                        Some(scope.name.clone())
                    };
                    if self.rename_app.is_some() {
                        let id = ui::keyboard_view::app_rename_input_id();
                        return Task::batch([
                            cosmic::widget::text_input::focus(id.clone()),
                            cosmic::widget::text_input::select_all(id),
                        ]);
                    }
                }
            }
            Message::RenameAppInput(text) => {
                if self.rename_app.is_some() {
                    self.rename_app = Some(text);
                }
            }
            Message::RenameAppCommit => self.commit_app_rename(),
            Message::NormalKeyHere => self.set_normal_here(),
            Message::EditMapping(index) => {
                if self.view != View::Keyboard {
                    return Task::none();
                }
                let Some((code, mapping)) = self.maps().get(index).cloned() else {
                    return Task::none();
                };
                let Some(cap) = model::key(&code) else {
                    return Task::none();
                };
                // Show the mapping's own scope, so the editor changes
                // it rather than adding an override of it.
                self.leave_layers();
                self.leave_apps();
                self.app_scope = (!mapping.app.is_empty()).then(|| mapping.app.clone());
                self.apps_open |= self.app_scope.is_some();
                if !model::same_device(&mapping.device, &self.device) {
                    self.device = if model::every_device(&mapping.device) {
                        "all".to_owned()
                    } else {
                        mapping.device.clone()
                    };
                    self.refresh_layout();
                }
                return self.update(Message::SelectKey(cap.code));
            }
            Message::AddLayer => self.add_layer(),
            Message::ChooseLayerKey => {
                if self.view == View::Keyboard && self.active_layer().is_some() {
                    self.choosing_layer_key = true;
                    self.rename_layer = None;
                    self.close_sheet();
                }
            }
            Message::CancelLayerKey => self.cancel_layer_key(),
            Message::DeleteLayer => {
                if self.view == View::Keyboard
                    && !self.choosing_layer_key
                    && let Some(layer) = self.active_layer()
                {
                    self.confirm_delete_layer = Some(layer.id.clone());
                }
            }
            Message::DeleteLayerConfirm => {
                if let Some(id) = self.confirm_delete_layer.take() {
                    self.delete_layer(&id);
                }
            }
            Message::DeleteLayerCancel => self.confirm_delete_layer = None,
            Message::RenameLayerToggle => {
                if self.view == View::Keyboard
                    && let Some(layer) = self.active_layer()
                {
                    self.rename_layer = if self.rename_layer.is_some() {
                        None
                    } else {
                        Some(layer.name.clone())
                    };
                    if self.rename_layer.is_some() {
                        let id = ui::keyboard_view::layer_rename_input_id();
                        return Task::batch([
                            cosmic::widget::text_input::focus(id.clone()),
                            cosmic::widget::text_input::select_all(id),
                        ]);
                    }
                }
            }
            Message::RenameLayerInput(text) => {
                if self.rename_layer.is_some() {
                    self.rename_layer = Some(text);
                }
            }
            Message::RenameLayerCommit => self.commit_layer_rename(),
            Message::SelectKey(code) => {
                if self.view == View::Tester {
                    self.last = Some(LastKey {
                        code,
                        device: "Clicked in this preview".to_owned(),
                    });
                } else if self.choosing_layer_key {
                    self.set_layer_trigger(code);
                } else {
                    if self.layer.is_some()
                        && let Some((text, sub)) = self.layer_job_blocker(code)
                    {
                        self.flash(text, sub);
                        return Task::none();
                    }
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
                if let Some(selected) = self.selected
                    && self.layer.is_some()
                {
                    self.set_layer_job(selected, &action);
                } else if self.mode == Mode::Combo {
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
                self.flash("Combination removed", "applies automatically");
            }
            Message::ToggleSwap => {
                let Some(selected) = self.selected else {
                    return Task::none();
                };
                let Some(tap) = self
                    .mapping(selected)
                    .and_then(|mapping| mapping.tap.clone())
                else {
                    return Task::none();
                };
                self.undo = Some(Undo::Maps(self.profile_maps.clone()));
                let entry = self.scoped_entry(selected);
                entry.swap = !entry.swap;
                let swapped = entry.swap;
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
            Message::RemoveMapping(index) => {
                if self.view == View::Keyboard && self.remaps_open && index < self.maps().len() {
                    self.confirm_remove_mapping = Some(index);
                }
            }
            Message::RemoveMappingConfirm => {
                if let Some(index) = self.confirm_remove_mapping.take()
                    && self.view == View::Keyboard
                    && self.remaps_open
                    && index < self.maps().len()
                {
                    self.remove_mapping_at(index);
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
                        scope: String::new(),
                        enabled: true,
                        any_mod: false,
                        rules: Vec::new(),
                    });
                });
                self.flash(
                    "Group added",
                    "Add shortcuts, or limit it to an application.",
                );
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
                        "applies automatically",
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
                    self.flash("Shortcut removed", "applies automatically");
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
                        self.persist();
                    }
                    Some(Undo::Apps { apps, maps, groups }) => {
                        self.profile_apps = apps;
                        self.profile_maps = maps;
                        self.profile_groups = groups;
                        self.rename_app = None;
                        self.picker = None;
                        if self.active_app_scope().is_none() {
                            self.app_scope = None;
                        }
                        self.toast = None;
                        self.persist();
                    }
                    Some(Undo::Layers { layers, maps }) => {
                        self.profile_layers = layers;
                        self.profile_maps = maps;
                        self.choosing_layer_key = false;
                        self.rename_layer = None;
                        if self.active_layer().is_none() {
                            self.layer = None;
                        }
                        self.toast = None;
                        self.persist();
                    }
                    Some(Undo::Profile {
                        index,
                        profile,
                        maps,
                        layers,
                        apps,
                        groups,
                    }) => {
                        self.profile_maps.insert(profile.id.clone(), maps);
                        self.profile_layers.insert(profile.id.clone(), layers);
                        self.profile_apps.insert(profile.id.clone(), apps);
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
            Message::SetupToggleDetails => {
                if let Some(setup) = &mut self.setup {
                    setup.details = !setup.details;
                }
            }
            Message::OpenUrl(url) => {
                // xdg-open hands the page to the default browser; the
                // detached spawn keeps the browser out of Keyloom's
                // process tree.
                return cosmic::task::future(async move {
                    let mut command = std::process::Command::new("xdg-open");
                    command.arg(url);
                    Message::UrlOpened {
                        url,
                        opened: cosmic::process::spawn(command).await.is_some(),
                    }
                });
            }
            Message::UrlOpened { url, opened } => {
                if !opened {
                    self.flash("Could not open a browser", format!("Visit {url} yourself."));
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
                    self.leave_layers();
                    self.leave_apps();
                    self.profile_maps.insert(id.clone(), Vec::new());
                    self.profile_layers.insert(id.clone(), Vec::new());
                    self.profile_apps.insert(id.clone(), Vec::new());
                    self.profile_groups.insert(id, Vec::new());
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
            return Some(ui::overlays::setup_dialog(setup));
        }
        if self.about_open {
            return Some(ui::overlays::about_dialog());
        }
        if self.confirm_delete.is_some() {
            return Some(ui::overlays::delete_profile_dialog(self));
        }
        if self.confirm_delete_layer.is_some() {
            return Some(ui::overlays::delete_layer_dialog(self));
        }
        if self.confirm_delete_app.is_some() {
            return Some(ui::overlays::delete_app_dialog(self));
        }
        if self.confirm_reset_mappings.is_some() {
            return Some(ui::overlays::reset_mappings_dialog(self));
        }
        if self.confirm_remove_mapping.is_some() {
            return Some(ui::overlays::remove_mapping_dialog(self));
        }
        if let Some(picker) = &self.picker {
            return Some(ui::overlays::picker_dialog(self, picker));
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
        } else if self.confirm_delete_layer.is_some() {
            self.confirm_delete_layer = None;
        } else if self.confirm_delete_app.is_some() {
            self.confirm_delete_app = None;
        } else if self.confirm_reset_mappings.is_some() {
            self.confirm_reset_mappings = None;
        } else if self.confirm_remove_mapping.is_some() {
            self.confirm_remove_mapping = None;
        } else if self.picker.is_some() {
            self.picker = None;
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
        } else if self.rename_layer.is_some() {
            self.rename_layer = None;
        } else if self.rename_app.is_some() {
            self.rename_app = None;
        } else if self.choosing_layer_key {
            self.cancel_layer_key();
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
        app.confirm_remove_mapping = Some(0);
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
        let _ = app.update(Message::RemoveMapping(0));
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
        let _ = app.update(Message::RemoveMapping(0));
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
            let _ = app.update(Message::RemoveMapping(0));
            assert_eq!(app.confirm_remove_mapping, Some(0));
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
        let _ = app.update(Message::RemoveMapping(0));
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
            let _ = app.update(Message::RemoveMapping(0));
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
        let _ = app.update(Message::RemoveMapping(0));
        assert!(app.confirm_remove_mapping.is_none());

        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::RemoveMapping(0));
        assert!(
            app.confirm_remove_mapping.is_none(),
            "closed list ignores removal"
        );

        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::RemoveMapping(0));
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
        let yaml = crate::xremap::generate(app.rules(), |id| id.to_owned());
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
            crate::xremap::generate(app.rules(), |id| id.to_owned()),
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
                crate::xremap::generate(app.rules(), |id| id.to_owned()),
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
            &original.profile_state(),
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
        for starter in &starters {
            let id = &starter.profile.id;
            assert!(
                app.profiles.iter().any(|seeded| &seeded.id == id),
                "{} ships as a regular profile",
                starter.profile.name
            );
            assert_eq!(app.profile_maps.get(id), Some(&starter.maps));
            assert_eq!(app.profile_layers.get(id), Some(&starter.layers));
        }
    }

    #[test]
    fn starter_profiles_are_editable_like_any_other() {
        let mut app = app();
        let seeded = model::starter_profiles()
            .into_iter()
            .find(|starter| starter.profile.id == "laptop")
            .map(|starter| starter.maps)
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
        let starters: Vec<String> = model::starter_profiles()
            .into_iter()
            .map(|starter| starter.profile.id)
            .collect();
        for id in starters {
            let _ = app.update(Message::DeleteProfile(id));
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
    fn a_page_that_could_not_be_opened_is_shown_instead() {
        let mut app = app();
        let _ = app.update(Message::UrlOpened {
            url: setup::XREMAP_URL,
            opened: true,
        });
        assert!(app.toast.is_none(), "an opened page needs no comment");

        let _ = app.update(Message::UrlOpened {
            url: setup::XREMAP_URL,
            opened: false,
        });
        let toast = app.toast.as_ref().expect("the user is told where to go");
        assert!(toast.sub.contains(setup::XREMAP_URL));
    }

    #[test]
    fn setup_details_are_hidden_until_asked_for() {
        let mut app = app();
        let _ = app.update(Message::SetupToggleDetails);
        assert!(app.setup.is_none(), "nothing to toggle while closed");

        let _ = app.update(Message::MenuShowSetup);
        assert!(!app.setup.as_ref().unwrap().details);
        let _ = app.update(Message::SetupToggleDetails);
        assert!(app.setup.as_ref().unwrap().details);
        let _ = app.update(Message::SetupPage(SetupPage::Step(setup::Step::Service)));
        assert!(
            app.setup.as_ref().unwrap().details,
            "the choice carries across pages"
        );
        let _ = app.update(Message::SetupToggleDetails);
        assert!(!app.setup.as_ref().unwrap().details);

        // Reopening starts hidden again.
        let _ = app.update(Message::SetupToggleDetails);
        let _ = app.update(Message::SetupSkip);
        let _ = app.update(Message::MenuShowSetup);
        assert!(!app.setup.as_ref().unwrap().details);
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

    // --- Layers ---------------------------------------------------------

    fn layer<'a>(app: &'a App, id: &str) -> &'a Layer {
        app.layers()
            .iter()
            .find(|layer| layer.id == id)
            .expect("layer exists")
    }

    fn yaml(app: &App) -> String {
        crate::xremap::generate(app.rules(), |id| id.to_owned())
    }

    fn toast_text(app: &App) -> &str {
        app.toast.as_ref().map_or("", |toast| toast.text.as_str())
    }

    #[test]
    fn a_new_layer_waits_for_its_key_then_takes_jobs() {
        let mut app = app();
        let _ = app.update(Message::AddLayer);
        assert!(app.choosing_layer_key);
        assert!(app.layers_open);
        assert_eq!(app.layer.as_deref(), Some("layer-1"));
        assert_eq!(layer(&app, "layer-1").name, "Layer 1");
        assert!(layer(&app, "layer-1").trigger.is_empty());
        let apply_seq = app.apply_seq;

        // The next click chooses the key; it opens no editor.
        let _ = app.update(Message::SelectKey("CapsLock"));
        assert!(!app.choosing_layer_key);
        assert_eq!(layer(&app, "layer-1").trigger, "CapsLock");
        assert_eq!(app.selected, None);
        assert_eq!(toast_text(&app), "Hold Caps Lock for Layer 1");
        assert!(app.apply_seq > apply_seq, "the layer key is applied");

        // Now keys take jobs in the layer through the editor.
        let _ = app.update(Message::SelectKey("KeyH"));
        assert_eq!(app.selected, Some("KeyH"));
        let _ = app.update(Message::PickAction("Arrow Left".to_owned()));
        let job = layer(&app, "layer-1").key("KeyH").expect("job stored");
        assert_eq!(job.action, "Arrow Left");
        assert_eq!(job.device, "all");
        assert_eq!(toast_text(&app), "Caps Lock + H → Arrow Left");
        assert!(
            app.mapping("KeyH").is_none(),
            "the normal keys are untouched"
        );
        let generated = yaml(&app);
        assert!(generated.contains("      KEY_CAPSLOCK: KEY_BRL_DOT1\n"));
        assert!(generated.contains("      KEY_BRL_DOT1-KEY_H: KEY_LEFT\n"));

        // Replacing the job keeps one entry per key.
        let _ = app.update(Message::PickAction("Home".to_owned()));
        assert_eq!(layer(&app, "layer-1").keys.len(), 1);
        assert_eq!(layer(&app, "layer-1").key("KeyH").unwrap().action, "Home");

        // "Back to normal in this layer" removes it, undoably.
        let _ = app.update(Message::ClearKey);
        assert!(layer(&app, "layer-1").key("KeyH").is_none());
        assert_eq!(toast_text(&app), "H back to normal in Layer 1");
        let _ = app.update(Message::Undo);
        assert!(layer(&app, "layer-1").key("KeyH").is_some());
    }

    #[test]
    fn a_physical_press_chooses_the_layer_key_and_records_jobs() {
        let mut app = app();
        let device = PathBuf::from("/dev/input/test-keyboard");
        let _ = app.update(Message::AddLayer);
        app.phys_press(&device, evdev::KeyCode::KEY_SPACE.0);
        assert_eq!(layer(&app, "layer-1").trigger, "Space");
        assert!(!app.choosing_layer_key);

        // Recording a key while editing a job records it in the layer.
        let _ = app.update(Message::SelectKey("KeyJ"));
        let _ = app.update(Message::SetCapture(true));
        app.phys_press(&device, evdev::KeyCode::KEY_DOWN.0);
        assert!(!app.capture);
        assert_eq!(
            layer(&app, "layer-1").key("KeyJ").unwrap().action,
            "Arrow Down"
        );
        assert!(app.mapping("KeyJ").is_none());
    }

    #[test]
    fn cancelling_the_key_choice_drops_only_a_new_layer() {
        let mut app = app();
        let _ = app.update(Message::AddLayer);
        let _ = app.on_escape();
        assert!(app.layers().is_empty(), "a layer without a key is dropped");
        assert_eq!(app.layer, None);
        assert!(!app.choosing_layer_key);

        // Showing the normal keys mid-choice does the same.
        let _ = app.update(Message::AddLayer);
        let _ = app.update(Message::SetLayer(None));
        assert!(app.layers().is_empty());

        // An existing layer keeps its key when a new choice is cancelled.
        let _ = app.update(Message::AddLayer);
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::ChooseLayerKey);
        assert!(app.choosing_layer_key);
        let _ = app.update(Message::CancelLayerKey);
        assert_eq!(app.layers().len(), 1);
        assert_eq!(layer(&app, "layer-1").trigger, "CapsLock");
        assert!(!app.choosing_layer_key);
    }

    #[test]
    fn a_layer_key_and_modifiers_take_no_jobs() {
        let mut app = app();
        let _ = app.update(Message::AddLayer);
        let _ = app.update(Message::SelectKey("CapsLock"));
        for (code, text) in [
            ("CapsLock", "Caps Lock holds this layer"),
            ("ShiftLeft", "Left Shift keeps its job"),
        ] {
            let _ = app.update(Message::SelectKey(code));
            assert_eq!(app.selected, None, "{code} opens no editor");
            assert_eq!(toast_text(&app), text);
        }

        // One key holds one layer, and a layer's key takes no job in
        // another layer.
        let _ = app.update(Message::AddLayer);
        let _ = app.update(Message::SelectKey("CapsLock"));
        assert!(app.choosing_layer_key, "the key is taken; keep choosing");
        assert_eq!(toast_text(&app), "Caps Lock already holds Layer 1");
        let _ = app.update(Message::SelectKey("Space"));
        assert_eq!(layer(&app, "layer-2").trigger, "Space");
        let _ = app.update(Message::SetLayer(Some("layer-1".to_owned())));
        let _ = app.update(Message::SelectKey("Space"));
        assert_eq!(app.selected, None);
        assert_eq!(toast_text(&app), "Space holds the Layer 2 layer");
    }

    #[test]
    fn holding_a_layer_supersedes_the_hold_action() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("laptop".to_owned()));
        assert_eq!(
            app.mapping("CapsLock").and_then(|m| m.hold.as_deref()),
            Some("Left Control")
        );
        let _ = app.update(Message::AddLayer);
        let _ = app.update(Message::SelectKey("CapsLock"));
        let mapping = app.mapping("CapsLock").expect("the tap action stays");
        assert_eq!(mapping.tap.as_deref(), Some("Escape"));
        assert_eq!(mapping.hold, None, "the hold action gave way to the layer");
        assert!(
            app.toast
                .as_ref()
                .is_some_and(|toast| toast.sub.contains("hold action gave way"))
        );
        assert!(
            yaml(&app).contains(
                "      KEY_CAPSLOCK:\n        held: KEY_BRL_DOT1\n        alone: KEY_ESC\n"
            )
        );

        // Undo puts both back, and the key-less layer is not resurrected.
        let _ = app.update(Message::Undo);
        assert!(app.layers().is_empty());
        assert_eq!(
            app.mapping("CapsLock").and_then(|m| m.hold.as_deref()),
            Some("Left Control")
        );
    }

    #[test]
    fn a_layer_key_refuses_new_hold_actions_but_may_tap_itself() {
        let mut app = app();
        let _ = app.update(Message::AddLayer);
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::SetLayer(None));
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::SetMode(Mode::Hold));
        let _ = app.update(Message::PickAction("Left Control".to_owned()));
        assert!(app.mapping("CapsLock").is_none());
        assert_eq!(toast_text(&app), "Caps Lock holds the Layer 1 layer");

        // Tapping the key as itself now means something: holding it is
        // the layer.
        let _ = app.update(Message::SetMode(Mode::Tap));
        let _ = app.update(Message::PickAction("Caps Lock".to_owned()));
        assert_eq!(
            app.mapping("CapsLock").and_then(|m| m.tap.as_deref()),
            Some("Caps Lock")
        );
        assert!(yaml(&app).contains("        alone: KEY_CAPSLOCK\n"));
    }

    #[test]
    fn deleting_a_layer_asks_first_and_is_undoable() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("navigation".to_owned()));
        let _ = app.update(Message::SetLayer(Some("navigation".to_owned())));
        assert!(app.active_layer().is_some());
        let _ = app.update(Message::DeleteLayer);
        assert_eq!(app.confirm_delete_layer.as_deref(), Some("navigation"));
        assert_eq!(app.layers().len(), 1, "asking changes nothing");
        let _ = app.on_escape();
        assert_eq!(app.confirm_delete_layer, None);
        assert_eq!(app.layers().len(), 1);

        let _ = app.update(Message::DeleteLayer);
        let _ = app.update(Message::DeleteLayerConfirm);
        assert!(app.layers().is_empty());
        assert_eq!(app.layer, None);
        assert_eq!(toast_text(&app), "Navigation deleted");
        assert!(!yaml(&app).contains("virtual_modifiers"));

        let _ = app.update(Message::Undo);
        assert_eq!(app.layers().len(), 1);
        assert_eq!(layer(&app, "navigation").keys.len(), 10);
    }

    #[test]
    fn renaming_the_active_layer() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("navigation".to_owned()));
        let _ = app.update(Message::SetLayer(Some("navigation".to_owned())));
        let _ = app.update(Message::RenameLayerToggle);
        assert_eq!(app.rename_layer.as_deref(), Some("Navigation"));
        let _ = app.update(Message::RenameLayerInput("  Vim keys ".to_owned()));
        let _ = app.update(Message::RenameLayerCommit);
        assert_eq!(app.rename_layer, None);
        assert_eq!(layer(&app, "navigation").name, "Vim keys");
        assert!(yaml(&app).contains("'Keyloom layer: Vim keys, hold Caps Lock'"));

        // Escape cancels an edit in progress.
        let _ = app.update(Message::RenameLayerToggle);
        let _ = app.update(Message::RenameLayerInput("Other".to_owned()));
        let _ = app.on_escape();
        assert_eq!(app.rename_layer, None);
        assert_eq!(layer(&app, "navigation").name, "Vim keys");
    }

    #[test]
    fn leaving_the_keyboard_view_or_profile_shows_the_normal_keys_again() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("navigation".to_owned()));
        let _ = app.update(Message::SetLayer(Some("navigation".to_owned())));
        let _ = app.update(Message::SetView(View::Tester));
        assert_eq!(app.layer, None);
        let _ = app.update(Message::SetView(View::Keyboard));
        let _ = app.update(Message::SetLayer(Some("navigation".to_owned())));
        assert!(app.active_layer().is_some());
        let _ = app.update(Message::SelectProfile("default".to_owned()));
        assert_eq!(app.layer, None);
        // A layer id from another profile is ignored.
        let _ = app.update(Message::SetLayer(Some("navigation".to_owned())));
        assert_eq!(app.layer, None);
    }

    #[test]
    fn the_navigation_starter_profile_generates_a_working_layer() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("navigation".to_owned()));
        let generated = yaml(&app);
        assert!(
            generated.contains(
                "      KEY_CAPSLOCK:\n        held: KEY_BRL_DOT1\n        alone: KEY_ESC\n"
            )
        );
        assert!(generated.contains("virtual_modifiers:\n  - KEY_BRL_DOT1\n"));
        for rule in [
            "KEY_BRL_DOT1-KEY_A: KEY_HOME",
            "KEY_BRL_DOT1-KEY_D: KEY_PAGEDOWN",
            "KEY_BRL_DOT1-KEY_E: KEY_END",
            "KEY_BRL_DOT1-KEY_H: KEY_LEFT",
            "KEY_BRL_DOT1-KEY_J: KEY_DOWN",
            "KEY_BRL_DOT1-KEY_K: KEY_UP",
            "KEY_BRL_DOT1-KEY_L: KEY_RIGHT",
            "KEY_BRL_DOT1-KEY_N: KEY_DELETE",
            "KEY_BRL_DOT1-KEY_U: KEY_PAGEUP",
            "KEY_BRL_DOT1-KEY_Y: KEY_BACKSPACE",
        ] {
            assert!(generated.contains(&format!("      {rule}\n")), "{rule}");
        }
    }

    #[test]
    fn profiles_carry_their_layers_through_duplicate_reset_and_delete() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("navigation".to_owned()));
        let _ = app.update(Message::NewProfile { duplicate: true });
        assert_eq!(app.profile_name(), "Navigation layer copy");
        assert_eq!(app.layers().len(), 1);
        assert_eq!(app.layer, None);

        let _ = app.update(Message::MenuReset);
        let _ = app.update(Message::ResetMappingsConfirm);
        assert!(app.layers().is_empty());
        assert!(app.maps().is_empty());

        // Deleting a profile takes its layers; Undo brings them back.
        let _ = app.update(Message::SelectProfile("default".to_owned()));
        let _ = app.update(Message::DeleteProfile("navigation".to_owned()));
        let _ = app.update(Message::DeleteConfirm);
        assert!(!app.profile_layers.contains_key("navigation"));
        let _ = app.update(Message::Undo);
        assert_eq!(app.profile_layers.get("navigation").map(Vec::len), Some(1));
    }

    #[test]
    fn layer_jobs_follow_the_device_scope() {
        let mut app = app();
        let _ = app.update(connected(
            "/dev/input/event1",
            "Laptop",
            keyboard::FORM_SIXTY_FIVE,
            false,
        ));
        let _ = app.update(Message::AddLayer);
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::SelectDevice("/dev/input/event1".to_owned()));
        let _ = app.update(Message::SelectKey("KeyH"));
        let _ = app.update(Message::PickAction("Arrow Left".to_owned()));
        assert_eq!(
            layer(&app, "layer-1").key("KeyH").unwrap().device,
            "/dev/input/event1"
        );
        assert!(
            crate::xremap::generate(app.rules(), |id| app.device_label(id)).contains(
                "    device:\n      only: ['Laptop']\n    remap:\n      KEY_BRL_DOT1-KEY_H: KEY_LEFT\n"
            )
        );
    }

    #[test]
    fn the_layer_pool_is_the_limit() {
        let mut app = app();
        for key in ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10"] {
            let _ = app.update(Message::AddLayer);
            let _ = app.update(Message::SelectKey(key));
        }
        assert_eq!(app.layers().len(), crate::xremap::MAX_LAYERS);
        let _ = app.update(Message::AddLayer);
        assert_eq!(app.layers().len(), crate::xremap::MAX_LAYERS);
        assert!(!app.choosing_layer_key);
        assert_eq!(toast_text(&app), "Up to 10 layers per profile");
    }

    #[test]
    fn the_tester_ignores_layer_editing() {
        let mut app = app();
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::AddLayer);
        assert!(app.layers().is_empty());
        assert!(!app.choosing_layer_key);
        let _ = app.update(Message::SetLayer(Some("navigation".to_owned())));
        assert_eq!(app.layer, None);
    }

    // --- Application scopes ----------------------------------------------

    /// Add an application scope through the picker by typing an id:
    /// tests cannot ask the desktop.
    fn add_terminal(app: &mut App) -> String {
        let _ = app.update(Message::AddAppScope);
        assert!(app.picker.is_some());
        let _ = app.update(Message::PickerCustom("com.system76.CosmicTerm".to_owned()));
        let _ = app.update(Message::PickerAddCustom);
        let _ = app.update(Message::PickerConfirm);
        app.app_scope.clone().expect("the new scope is shown")
    }

    #[test]
    fn an_application_scope_is_added_from_the_picker_and_shown_on_the_deck() {
        let mut app = app();
        // Nothing chosen: the picker stays open until cancelled.
        let _ = app.update(Message::AddAppScope);
        let _ = app.update(Message::PickerConfirm);
        assert!(app.picker.is_some());
        let _ = app.on_escape();
        assert!(app.picker.is_none());

        let id = add_terminal(&mut app);
        assert_eq!(id, "app-1");
        assert!(app.picker.is_none());
        assert!(app.apps_open);
        let scope = &app.app_scopes()[0];
        assert_eq!(scope.name, "com.system76.CosmicTerm");
        assert_eq!(scope.apps[0].id, "com.system76.CosmicTerm");
        assert_eq!(toast_text(&app), "com.system76.CosmicTerm added");
        assert_eq!(app.here_label(), "in com.system76.CosmicTerm");
        assert!(app.in_specific_scope());

        // The tester has no use for the picker.
        let _ = app.update(Message::SetView(View::Tester));
        let _ = app.update(Message::AddAppScope);
        assert!(app.picker.is_none());
    }

    #[test]
    fn a_mapping_in_an_application_overrides_the_general_one_only_there() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::SetMode(Mode::Hold));
        let _ = app.update(Message::PickAction("Left Control".to_owned()));
        let _ = app.update(Message::ClosePanel);

        let id = add_terminal(&mut app);
        // The general remap shows through, inherited.
        let inherited = app.effective_mapping("CapsLock").expect("inherited");
        assert!(!inherited.own);
        assert_eq!(inherited.mapping.tap.as_deref(), Some("Escape"));
        assert!(app.own_mapping("CapsLock").is_none());
        assert_eq!(app.inherited_from(inherited.mapping), "all applications");
        assert_eq!(app.other_scopes("CapsLock"), 0);

        // An override starts from the inherited mapping, so the hold
        // action survives the change of the tap.
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Tab".to_owned()));
        let own = app
            .own_mapping("CapsLock")
            .expect("own mapping in the terminal");
        assert_eq!(own.tap.as_deref(), Some("Tab"));
        assert_eq!(own.hold.as_deref(), Some("Left Control"));
        assert_eq!(own.app, id);
        assert_eq!(toast_text(&app), "Caps Lock → Tab");
        assert!(
            app.toast
                .as_ref()
                .unwrap()
                .sub
                .contains("in com.system76.CosmicTerm")
        );
        assert_eq!(app.maps().len(), 2, "the general mapping is untouched");

        // Back to every application: the general mapping applies, and
        // the key is marked as differing elsewhere.
        let _ = app.update(Message::SetAppScope(None));
        assert_eq!(
            app.mapping("CapsLock").and_then(|m| m.tap.as_deref()),
            Some("Escape")
        );
        assert_eq!(app.other_scopes("CapsLock"), 1);

        let yaml = yaml(&app);
        assert!(
            yaml.contains(
                "    application:\n      only: ['com.system76.CosmicTerm']\n    remap:\n      KEY_CAPSLOCK:\n        held: KEY_LEFTCTRL\n        alone: KEY_TAB\n"
            ),
            "{yaml}"
        );
    }

    #[test]
    fn a_normal_key_in_an_application_stands_in_for_the_general_remap() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        // Not in a specific scope: nothing to keep normal.
        let _ = app.update(Message::NormalKeyHere);
        assert!(app.maps().iter().all(|(_, m)| !m.normal));
        let _ = app.update(Message::ClosePanel);

        add_terminal(&mut app);
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::NormalKeyHere);
        let own = app.own_mapping("CapsLock").expect("normal here");
        assert!(own.normal);
        assert_eq!(own.tap, None);
        assert_eq!(toast_text(&app), "Caps Lock stays Caps Lock");
        assert!(yaml(&app).contains("      KEY_CAPSLOCK: KEY_CAPSLOCK\n"));
        let apply_seq = app.apply_seq;
        let _ = app.update(Message::NormalKeyHere);
        assert_eq!(app.apply_seq, apply_seq, "nothing to change");
        assert_eq!(toast_text(&app), "Caps Lock is already a normal key here");

        // Choosing an action replaces the exception; clearing the key
        // drops the override and the general remap shows through again.
        let _ = app.update(Message::PickAction("Tab".to_owned()));
        assert!(!app.own_mapping("CapsLock").unwrap().normal);
        let _ = app.update(Message::ClearKey);
        assert!(app.own_mapping("CapsLock").is_none());
        assert_eq!(
            app.mapping("CapsLock").and_then(|m| m.tap.as_deref()),
            Some("Escape")
        );
        assert_eq!(toast_text(&app), "Caps Lock same as all applications");
        // Nothing of its own left: clearing again only explains, and
        // the earlier removal stays undoable.
        let _ = app.update(Message::ClearKey);
        assert_eq!(toast_text(&app), "Caps Lock has no remap of its own here");
        let _ = app.update(Message::Undo);
        assert_eq!(
            app.own_mapping("CapsLock").and_then(|m| m.tap.as_deref()),
            Some("Tab")
        );
    }

    #[test]
    fn removing_an_application_takes_its_remaps_and_is_undoable() {
        let mut app = app();
        add_terminal(&mut app);
        let _ = app.update(Message::SelectKey("KeyA"));
        let _ = app.update(Message::PickAction("B".to_owned()));
        let _ = app.update(Message::SetMode(Mode::Combo));
        let _ = app.update(Message::PickAction("C".to_owned()));
        assert_eq!(app.groups().len(), 1);
        assert_eq!(app.groups()[0].id, "kb:app-1");
        assert_eq!(app.groups()[0].scope, "app-1");
        assert_eq!(app.scope_count("app-1"), 2);

        let _ = app.update(Message::DeleteAppScope);
        assert_eq!(app.confirm_delete_app.as_deref(), Some("app-1"));
        let _ = app.on_escape();
        assert!(app.confirm_delete_app.is_none());
        assert_eq!(app.app_scopes().len(), 1);

        let _ = app.update(Message::DeleteAppScope);
        let _ = app.update(Message::DeleteAppScopeConfirm);
        assert!(app.app_scopes().is_empty());
        assert!(app.maps().is_empty());
        assert!(app.groups().is_empty());
        assert_eq!(app.app_scope, None);
        assert_eq!(toast_text(&app), "com.system76.CosmicTerm removed");

        let _ = app.update(Message::Undo);
        assert_eq!(app.app_scopes().len(), 1);
        assert_eq!(app.maps().len(), 1);
        assert_eq!(app.groups().len(), 1);
    }

    #[test]
    fn one_scope_per_application_and_the_picker_edits_a_scope() {
        let mut app = app();
        add_terminal(&mut app);
        // A second scope cannot take the same application.
        let _ = app.update(Message::SetAppScope(None));
        let _ = app.update(Message::AddAppScope);
        let _ = app.update(Message::PickerCustom("com.system76.CosmicTerm".to_owned()));
        let _ = app.update(Message::PickerAddCustom);
        assert!(app.picker.as_ref().unwrap().chosen.is_empty());
        assert_eq!(
            toast_text(&app),
            "com.system76.CosmicTerm already has its own remaps"
        );
        let _ = app.update(Message::PickerCustom("firefox".to_owned()));
        let _ = app.update(Message::PickerAddCustom);
        let _ = app.update(Message::PickerConfirm);
        assert_eq!(app.app_scopes().len(), 2);
        assert_eq!(app.app_scope.as_deref(), Some("app-2"));

        // Renaming and changing the shown scope's applications.
        let _ = app.update(Message::RenameAppToggle);
        let _ = app.update(Message::RenameAppInput("Browsers".to_owned()));
        let _ = app.update(Message::RenameAppCommit);
        assert_eq!(app.active_app_scope().unwrap().name, "Browsers");
        let _ = app.update(Message::ChangeApps);
        let picker = app.picker.as_ref().unwrap();
        assert_eq!(picker.target, PickerTarget::Scope("app-2".to_owned()));
        assert_eq!(picker.chosen.len(), 1);
        let _ = app.update(Message::PickerCustom("chromium".to_owned()));
        let _ = app.update(Message::PickerAddCustom);
        // Choosing an application again unchooses it.
        let _ = app.update(Message::PickerToggle(AppRef {
            id: "firefox".to_owned(),
            name: "firefox".to_owned(),
            aliases: Vec::new(),
        }));
        assert_eq!(app.picker.as_ref().unwrap().chosen.len(), 1);
        let _ = app.update(Message::PickerConfirm);
        let scope = app.active_app_scope().unwrap();
        assert_eq!(scope.name, "Browsers", "the name is kept");
        assert_eq!(scope.apps.len(), 1);
        assert_eq!(scope.apps[0].id, "chromium");
        assert_eq!(toast_text(&app), "Browsers updated");
    }

    #[test]
    fn layers_and_applications_are_shown_one_at_a_time_and_leave_with_the_view() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("navigation".to_owned()));
        add_terminal(&mut app);
        let _ = app.update(Message::SetLayer(Some("navigation".to_owned())));
        assert_eq!(
            app.app_scope, None,
            "a layer shows the same jobs in every application"
        );
        assert!(app.layer.is_some());
        let _ = app.update(Message::SetAppScope(Some("app-1".to_owned())));
        assert_eq!(app.layer, None);
        assert_eq!(app.app_scope.as_deref(), Some("app-1"));
        let _ = app.update(Message::AddLayer);
        assert_eq!(app.app_scope, None);
        let _ = app.update(Message::CancelLayerKey);

        let _ = app.update(Message::SetAppScope(Some("app-1".to_owned())));
        let _ = app.update(Message::SetView(View::Tester));
        assert_eq!(app.app_scope, None);
        let _ = app.update(Message::SetView(View::Keyboard));
        let _ = app.update(Message::SetAppScope(Some("app-1".to_owned())));
        let _ = app.update(Message::SelectProfile("default".to_owned()));
        assert_eq!(app.app_scope, None);
        // An unknown scope is not shown.
        let _ = app.update(Message::SetAppScope(Some("app-9".to_owned())));
        assert_eq!(app.app_scope, None);
    }

    #[test]
    fn profiles_carry_their_applications_through_duplicate_reset_and_delete() {
        let mut app = app();
        add_terminal(&mut app);
        let _ = app.update(Message::SelectKey("KeyA"));
        let _ = app.update(Message::PickAction("B".to_owned()));
        let _ = app.update(Message::NewProfile { duplicate: true });
        assert_eq!(app.app_scopes().len(), 1);
        assert_eq!(app.app_scope, None);
        assert_eq!(app.maps().len(), 1);

        let _ = app.update(Message::MenuReset);
        let _ = app.update(Message::ResetMappingsConfirm);
        assert!(app.app_scopes().is_empty());
        assert!(app.maps().is_empty());

        let _ = app.update(Message::SelectProfile("default".to_owned()));
        assert_eq!(app.app_scopes().len(), 1, "the original keeps its scope");
        let _ = app.update(Message::DeleteProfile("custom-1".to_owned()));
        let _ = app.update(Message::DeleteConfirm);
        assert!(!app.profile_apps.contains_key("custom-1"));
        let _ = app.update(Message::Undo);
        assert!(app.profile_apps.contains_key("custom-1"));

        let state = app.profile_state();
        assert_eq!(state.apps.get("default").map(Vec::len), Some(1));
    }

    #[test]
    fn the_remaps_list_opens_a_mapping_in_its_own_scope() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::ClosePanel);
        add_terminal(&mut app);
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Tab".to_owned()));
        let _ = app.update(Message::ClosePanel);
        let _ = app.update(Message::SetAppScope(None));
        assert_eq!(app.maps().len(), 2);
        let terminal = app
            .maps()
            .iter()
            .position(|(_, m)| m.app == "app-1")
            .unwrap();

        let _ = app.update(Message::OpenRemaps);
        let _ = app.update(Message::EditMapping(terminal));
        assert_eq!(app.app_scope.as_deref(), Some("app-1"));
        assert_eq!(app.selected, Some("CapsLock"));
        assert!(!app.remaps_open);
        assert_eq!(
            app.own_mapping("CapsLock").and_then(|m| m.tap.as_deref()),
            Some("Tab")
        );

        // Removing from the list takes exactly that entry.
        let _ = app.update(Message::ClosePanel);
        let _ = app.update(Message::OpenRemaps);
        let _ = app.update(Message::RemoveMapping(terminal));
        let _ = app.update(Message::RemoveMappingConfirm);
        assert_eq!(app.maps().len(), 1);
        assert!(app.maps()[0].1.app.is_empty());
        assert_eq!(toast_text(&app), "Caps Lock same as all applications");
    }

    #[test]
    fn a_layer_key_kept_normal_in_an_application_switches_the_layer_off_there() {
        let mut app = app();
        let _ = app.update(Message::SelectProfile("navigation".to_owned()));
        add_terminal(&mut app);
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::NormalKeyHere);
        let yaml = yaml(&app);
        let normal = yaml
            .find("      KEY_CAPSLOCK: KEY_CAPSLOCK\n")
            .expect("normal in the terminal");
        let held = yaml
            .find("      KEY_CAPSLOCK:\n        held: KEY_BRL_DOT1\n        alone: KEY_ESC\n")
            .expect("holds the layer elsewhere");
        assert!(normal < held);
    }

    #[test]
    fn a_keyboard_inherits_the_general_remap_and_can_override_it() {
        let mut app = app();
        let _ = app.update(connected(
            "/dev/input/event1",
            "Laptop",
            keyboard::FORM_SIXTY_FIVE,
            false,
        ));
        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Escape".to_owned()));
        let _ = app.update(Message::ClosePanel);
        let _ = app.update(Message::SelectDevice("/dev/input/event1".to_owned()));
        let inherited = app.effective_mapping("CapsLock").unwrap();
        assert!(!inherited.own);
        assert_eq!(app.inherited_from(inherited.mapping), "all keyboards");
        assert_eq!(app.here_label(), "on Laptop");

        let _ = app.update(Message::SelectKey("CapsLock"));
        let _ = app.update(Message::PickAction("Tab".to_owned()));
        assert_eq!(app.maps().len(), 2);
        assert_eq!(
            app.own_mapping("CapsLock").unwrap().device,
            "/dev/input/event1"
        );
        let _ = app.update(Message::SelectDevice("all".to_owned()));
        assert_eq!(
            app.mapping("CapsLock").unwrap().tap.as_deref(),
            Some("Escape")
        );
        assert_eq!(app.other_scopes("CapsLock"), 1);
    }

    #[test]
    fn shortcut_groups_take_an_application_scope_and_persist_with_the_profile() {
        let mut app = app();
        add_terminal(&mut app);
        let _ = app.update(Message::SetView(View::Shortcuts));
        let _ = app.update(Message::AddGroup);
        let apply_seq = app.apply_seq;
        assert_eq!(app.groups().len(), 1);
        let _ = app.update(Message::SetGroupScope {
            group: 0,
            scope: "app-1".to_owned(),
        });
        assert_eq!(app.groups()[0].scope, "app-1");
        assert_eq!(toast_text(&app), "New group · com.system76.CosmicTerm");
        // An unknown scope is refused.
        let _ = app.update(Message::SetGroupScope {
            group: 0,
            scope: "app-9".to_owned(),
        });
        assert_eq!(app.groups()[0].scope, "app-1");

        // Recording a complete rule generates a scoped keymap block.
        let _ = app.update(Message::EditRule {
            group: 0,
            rule: None,
        });
        let device = PathBuf::from("/dev/input/test");
        app.pressed
            .insert((device.clone(), evdev::KeyCode::KEY_LEFTMETA.0));
        app.phys_press(&device, evdev::KeyCode::KEY_C.0);
        let _ = app.update(Message::SetRecording(Some(Side::To)));
        app.pressed
            .remove(&(device.clone(), evdev::KeyCode::KEY_LEFTMETA.0));
        app.pressed
            .insert((device.clone(), evdev::KeyCode::KEY_LEFTCTRL.0));
        app.pressed
            .insert((device.clone(), evdev::KeyCode::KEY_LEFTSHIFT.0));
        app.phys_press(&device, evdev::KeyCode::KEY_C.0);
        let rule = &app.groups()[0].rules[0];
        assert_eq!(rule.from.mods, vec!["Super".to_owned()]);
        assert_eq!(rule.to.mods, vec!["Ctrl".to_owned(), "Shift".to_owned()]);
        assert!(app.apply_seq > apply_seq, "shortcut changes apply");
        let yaml = yaml(&app);
        assert!(
            yaml.contains(
                "  - name: 'Keyloom shortcuts: New group (com.system76.CosmicTerm)'\n    application:\n      only: ['com.system76.CosmicTerm']\n    remap:\n      Super-KEY_C: Ctrl-Shift-KEY_C\n"
            ),
            "{yaml}"
        );

        // Groups are saved with the profile.
        let state = app.profile_state();
        assert_eq!(state.groups.get("default").map(Vec::len), Some(1));

        // A new application for a group is made from the picker.
        let _ = app.update(Message::GroupAppScope(0));
        assert_eq!(app.picker.as_ref().unwrap().target, PickerTarget::Group(0));
        let _ = app.update(Message::PickerCustom("firefox".to_owned()));
        let _ = app.update(Message::PickerAddCustom);
        let _ = app.update(Message::PickerConfirm);
        assert_eq!(app.groups()[0].scope, "app-2");
        assert_eq!(app.app_scopes().len(), 2);
    }

    #[test]
    fn combos_follow_the_shown_application_scope() {
        let mut app = app();
        let _ = app.update(Message::SelectKey("KeyC"));
        let _ = app.update(Message::SetMode(Mode::Combo));
        let _ = app.update(Message::PickAction("Copy".to_owned()));
        let _ = app.update(Message::ClosePanel);
        add_terminal(&mut app);
        assert_eq!(
            app.combos_for("C").len(),
            1,
            "general combos apply in the terminal too"
        );
        let _ = app.update(Message::SelectKey("KeyC"));
        let _ = app.update(Message::SetMode(Mode::Combo));
        let _ = app.update(Message::PickAction("Paste".to_owned()));
        assert_eq!(app.groups().len(), 2);
        assert_eq!(app.combos_for("C").len(), 2);
        assert!(
            app.toast
                .as_ref()
                .unwrap()
                .sub
                .contains("com.system76.CosmicTerm")
        );
        let _ = app.update(Message::SetAppScope(None));
        assert_eq!(
            app.combos_for("C").len(),
            1,
            "the terminal's combo is not shown everywhere"
        );
    }
}

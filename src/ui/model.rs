//! UI data model ported from the design export's prototype script:
//! key geometry, the action catalog, demo profiles, and shortcut groups.
//!
//! This backs the GUI only — nothing here touches the system's real
//! keymap yet.

use evdev::KeyCode;
use serde::{Deserialize, Serialize};

/// One key unit in the deck, in logical pixels (`U` in the export).
pub const UNIT: f32 = 49.0;
/// Full deck size for the 100% layout.
pub const DECK_WIDTH: f32 = 1099.0;
pub const DECK_HEIGHT: f32 = 313.0;

/// The modifier chips offered by the combo editors.
pub const MODS: [&str; 4] = ["Ctrl", "Shift", "Alt", "Super"];

/// One physical key cap on the rendered keyboard.
#[derive(Clone, Copy, Debug)]
pub struct KeyCap {
    /// Stable identifier, using web `KeyboardEvent.code` names as in the
    /// design export (e.g. `KeyA`, `CapsLock`).
    pub code: &'static str,
    /// Printed cap legend.
    pub label: &'static str,
    /// Linux input-event scancode produced by this key.
    pub evdev: u16,
    /// Position and size in key units, relative to the block offset.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Horizontal block offset in logical pixels (main / nav / numpad).
    pub gx: f32,
}

/// Blocks a key can belong to, used by the TKL filter in the export.
pub const NUMPAD_GX: f32 = 908.0;
const NAV_GX: f32 = 748.0;

macro_rules! caps {
    ($($code:literal, $label:literal, $ev:expr, $x:expr, $y:expr, $w:expr, $h:expr, $gx:expr;)*) => {
        &[$(KeyCap {
            code: $code,
            label: $label,
            evdev: $ev.0,
            x: $x,
            y: $y,
            w: $w,
            h: $h,
            gx: $gx,
        },)*]
    };
}

/// Every key of the 100% ANSI deck, positioned exactly as in the export.
#[rustfmt::skip]
pub const ALL_KEYS: &[KeyCap] = caps![
    // Function row.
    "Escape", "Esc", KeyCode::KEY_ESC, 0.0, 0.0, 1.0, 1.0, 0.0;
    "F1", "F1", KeyCode::KEY_F1, 2.0, 0.0, 1.0, 1.0, 0.0;
    "F2", "F2", KeyCode::KEY_F2, 3.0, 0.0, 1.0, 1.0, 0.0;
    "F3", "F3", KeyCode::KEY_F3, 4.0, 0.0, 1.0, 1.0, 0.0;
    "F4", "F4", KeyCode::KEY_F4, 5.0, 0.0, 1.0, 1.0, 0.0;
    "F5", "F5", KeyCode::KEY_F5, 6.5, 0.0, 1.0, 1.0, 0.0;
    "F6", "F6", KeyCode::KEY_F6, 7.5, 0.0, 1.0, 1.0, 0.0;
    "F7", "F7", KeyCode::KEY_F7, 8.5, 0.0, 1.0, 1.0, 0.0;
    "F8", "F8", KeyCode::KEY_F8, 9.5, 0.0, 1.0, 1.0, 0.0;
    "F9", "F9", KeyCode::KEY_F9, 11.0, 0.0, 1.0, 1.0, 0.0;
    "F10", "F10", KeyCode::KEY_F10, 12.0, 0.0, 1.0, 1.0, 0.0;
    "F11", "F11", KeyCode::KEY_F11, 13.0, 0.0, 1.0, 1.0, 0.0;
    "F12", "F12", KeyCode::KEY_F12, 14.0, 0.0, 1.0, 1.0, 0.0;
    // Number row.
    "Backquote", "`", KeyCode::KEY_GRAVE, 0.0, 1.5, 1.0, 1.0, 0.0;
    "Digit1", "1", KeyCode::KEY_1, 1.0, 1.5, 1.0, 1.0, 0.0;
    "Digit2", "2", KeyCode::KEY_2, 2.0, 1.5, 1.0, 1.0, 0.0;
    "Digit3", "3", KeyCode::KEY_3, 3.0, 1.5, 1.0, 1.0, 0.0;
    "Digit4", "4", KeyCode::KEY_4, 4.0, 1.5, 1.0, 1.0, 0.0;
    "Digit5", "5", KeyCode::KEY_5, 5.0, 1.5, 1.0, 1.0, 0.0;
    "Digit6", "6", KeyCode::KEY_6, 6.0, 1.5, 1.0, 1.0, 0.0;
    "Digit7", "7", KeyCode::KEY_7, 7.0, 1.5, 1.0, 1.0, 0.0;
    "Digit8", "8", KeyCode::KEY_8, 8.0, 1.5, 1.0, 1.0, 0.0;
    "Digit9", "9", KeyCode::KEY_9, 9.0, 1.5, 1.0, 1.0, 0.0;
    "Digit0", "0", KeyCode::KEY_0, 10.0, 1.5, 1.0, 1.0, 0.0;
    "Minus", "-", KeyCode::KEY_MINUS, 11.0, 1.5, 1.0, 1.0, 0.0;
    "Equal", "=", KeyCode::KEY_EQUAL, 12.0, 1.5, 1.0, 1.0, 0.0;
    "Backspace", "⌫", KeyCode::KEY_BACKSPACE, 13.0, 1.5, 2.0, 1.0, 0.0;
    // Top letter row.
    "Tab", "Tab", KeyCode::KEY_TAB, 0.0, 2.5, 1.5, 1.0, 0.0;
    "KeyQ", "Q", KeyCode::KEY_Q, 1.5, 2.5, 1.0, 1.0, 0.0;
    "KeyW", "W", KeyCode::KEY_W, 2.5, 2.5, 1.0, 1.0, 0.0;
    "KeyE", "E", KeyCode::KEY_E, 3.5, 2.5, 1.0, 1.0, 0.0;
    "KeyR", "R", KeyCode::KEY_R, 4.5, 2.5, 1.0, 1.0, 0.0;
    "KeyT", "T", KeyCode::KEY_T, 5.5, 2.5, 1.0, 1.0, 0.0;
    "KeyY", "Y", KeyCode::KEY_Y, 6.5, 2.5, 1.0, 1.0, 0.0;
    "KeyU", "U", KeyCode::KEY_U, 7.5, 2.5, 1.0, 1.0, 0.0;
    "KeyI", "I", KeyCode::KEY_I, 8.5, 2.5, 1.0, 1.0, 0.0;
    "KeyO", "O", KeyCode::KEY_O, 9.5, 2.5, 1.0, 1.0, 0.0;
    "KeyP", "P", KeyCode::KEY_P, 10.5, 2.5, 1.0, 1.0, 0.0;
    "BracketLeft", "[", KeyCode::KEY_LEFTBRACE, 11.5, 2.5, 1.0, 1.0, 0.0;
    "BracketRight", "]", KeyCode::KEY_RIGHTBRACE, 12.5, 2.5, 1.0, 1.0, 0.0;
    "Backslash", "\\", KeyCode::KEY_BACKSLASH, 13.5, 2.5, 1.5, 1.0, 0.0;
    // Home row.
    "CapsLock", "Caps", KeyCode::KEY_CAPSLOCK, 0.0, 3.5, 1.75, 1.0, 0.0;
    "KeyA", "A", KeyCode::KEY_A, 1.75, 3.5, 1.0, 1.0, 0.0;
    "KeyS", "S", KeyCode::KEY_S, 2.75, 3.5, 1.0, 1.0, 0.0;
    "KeyD", "D", KeyCode::KEY_D, 3.75, 3.5, 1.0, 1.0, 0.0;
    "KeyF", "F", KeyCode::KEY_F, 4.75, 3.5, 1.0, 1.0, 0.0;
    "KeyG", "G", KeyCode::KEY_G, 5.75, 3.5, 1.0, 1.0, 0.0;
    "KeyH", "H", KeyCode::KEY_H, 6.75, 3.5, 1.0, 1.0, 0.0;
    "KeyJ", "J", KeyCode::KEY_J, 7.75, 3.5, 1.0, 1.0, 0.0;
    "KeyK", "K", KeyCode::KEY_K, 8.75, 3.5, 1.0, 1.0, 0.0;
    "KeyL", "L", KeyCode::KEY_L, 9.75, 3.5, 1.0, 1.0, 0.0;
    "Semicolon", ";", KeyCode::KEY_SEMICOLON, 10.75, 3.5, 1.0, 1.0, 0.0;
    "Quote", "'", KeyCode::KEY_APOSTROPHE, 11.75, 3.5, 1.0, 1.0, 0.0;
    "Enter", "Enter", KeyCode::KEY_ENTER, 12.75, 3.5, 2.25, 1.0, 0.0;
    // Bottom letter row.
    "ShiftLeft", "Shift", KeyCode::KEY_LEFTSHIFT, 0.0, 4.5, 2.25, 1.0, 0.0;
    "KeyZ", "Z", KeyCode::KEY_Z, 2.25, 4.5, 1.0, 1.0, 0.0;
    "KeyX", "X", KeyCode::KEY_X, 3.25, 4.5, 1.0, 1.0, 0.0;
    "KeyC", "C", KeyCode::KEY_C, 4.25, 4.5, 1.0, 1.0, 0.0;
    "KeyV", "V", KeyCode::KEY_V, 5.25, 4.5, 1.0, 1.0, 0.0;
    "KeyB", "B", KeyCode::KEY_B, 6.25, 4.5, 1.0, 1.0, 0.0;
    "KeyN", "N", KeyCode::KEY_N, 7.25, 4.5, 1.0, 1.0, 0.0;
    "KeyM", "M", KeyCode::KEY_M, 8.25, 4.5, 1.0, 1.0, 0.0;
    "Comma", ",", KeyCode::KEY_COMMA, 9.25, 4.5, 1.0, 1.0, 0.0;
    "Period", ".", KeyCode::KEY_DOT, 10.25, 4.5, 1.0, 1.0, 0.0;
    "Slash", "/", KeyCode::KEY_SLASH, 11.25, 4.5, 1.0, 1.0, 0.0;
    "ShiftRight", "Shift", KeyCode::KEY_RIGHTSHIFT, 12.25, 4.5, 2.75, 1.0, 0.0;
    // Modifier row.
    "ControlLeft", "Ctrl", KeyCode::KEY_LEFTCTRL, 0.0, 5.5, 1.25, 1.0, 0.0;
    "MetaLeft", "Super", KeyCode::KEY_LEFTMETA, 1.25, 5.5, 1.25, 1.0, 0.0;
    "AltLeft", "Alt", KeyCode::KEY_LEFTALT, 2.5, 5.5, 1.25, 1.0, 0.0;
    "Space", "", KeyCode::KEY_SPACE, 3.75, 5.5, 6.25, 1.0, 0.0;
    "AltRight", "Alt", KeyCode::KEY_RIGHTALT, 10.0, 5.5, 1.25, 1.0, 0.0;
    "MetaRight", "Super", KeyCode::KEY_RIGHTMETA, 11.25, 5.5, 1.25, 1.0, 0.0;
    "ContextMenu", "Menu", KeyCode::KEY_COMPOSE, 12.5, 5.5, 1.25, 1.0, 0.0;
    "ControlRight", "Ctrl", KeyCode::KEY_RIGHTCTRL, 13.75, 5.5, 1.25, 1.0, 0.0;
    // Navigation cluster.
    "PrintScreen", "PrtSc", KeyCode::KEY_SYSRQ, 0.0, 0.0, 1.0, 1.0, NAV_GX;
    "ScrollLock", "ScrLk", KeyCode::KEY_SCROLLLOCK, 1.0, 0.0, 1.0, 1.0, NAV_GX;
    "Pause", "Pause", KeyCode::KEY_PAUSE, 2.0, 0.0, 1.0, 1.0, NAV_GX;
    "Insert", "Ins", KeyCode::KEY_INSERT, 0.0, 1.5, 1.0, 1.0, NAV_GX;
    "Home", "Home", KeyCode::KEY_HOME, 1.0, 1.5, 1.0, 1.0, NAV_GX;
    "PageUp", "PgUp", KeyCode::KEY_PAGEUP, 2.0, 1.5, 1.0, 1.0, NAV_GX;
    "Delete", "Del", KeyCode::KEY_DELETE, 0.0, 2.5, 1.0, 1.0, NAV_GX;
    "End", "End", KeyCode::KEY_END, 1.0, 2.5, 1.0, 1.0, NAV_GX;
    "PageDown", "PgDn", KeyCode::KEY_PAGEDOWN, 2.0, 2.5, 1.0, 1.0, NAV_GX;
    "ArrowUp", "↑", KeyCode::KEY_UP, 1.0, 4.5, 1.0, 1.0, NAV_GX;
    "ArrowLeft", "←", KeyCode::KEY_LEFT, 0.0, 5.5, 1.0, 1.0, NAV_GX;
    "ArrowDown", "↓", KeyCode::KEY_DOWN, 1.0, 5.5, 1.0, 1.0, NAV_GX;
    "ArrowRight", "→", KeyCode::KEY_RIGHT, 2.0, 5.5, 1.0, 1.0, NAV_GX;
    // Numpad.
    "NumLock", "NumLk", KeyCode::KEY_NUMLOCK, 0.0, 1.5, 1.0, 1.0, NUMPAD_GX;
    "NumpadDivide", "/", KeyCode::KEY_KPSLASH, 1.0, 1.5, 1.0, 1.0, NUMPAD_GX;
    "NumpadMultiply", "*", KeyCode::KEY_KPASTERISK, 2.0, 1.5, 1.0, 1.0, NUMPAD_GX;
    "NumpadSubtract", "−", KeyCode::KEY_KPMINUS, 3.0, 1.5, 1.0, 1.0, NUMPAD_GX;
    "Numpad7", "7", KeyCode::KEY_KP7, 0.0, 2.5, 1.0, 1.0, NUMPAD_GX;
    "Numpad8", "8", KeyCode::KEY_KP8, 1.0, 2.5, 1.0, 1.0, NUMPAD_GX;
    "Numpad9", "9", KeyCode::KEY_KP9, 2.0, 2.5, 1.0, 1.0, NUMPAD_GX;
    "NumpadAdd", "+", KeyCode::KEY_KPPLUS, 3.0, 2.5, 1.0, 2.0, NUMPAD_GX;
    "Numpad4", "4", KeyCode::KEY_KP4, 0.0, 3.5, 1.0, 1.0, NUMPAD_GX;
    "Numpad5", "5", KeyCode::KEY_KP5, 1.0, 3.5, 1.0, 1.0, NUMPAD_GX;
    "Numpad6", "6", KeyCode::KEY_KP6, 2.0, 3.5, 1.0, 1.0, NUMPAD_GX;
    "Numpad1", "1", KeyCode::KEY_KP1, 0.0, 4.5, 1.0, 1.0, NUMPAD_GX;
    "Numpad2", "2", KeyCode::KEY_KP2, 1.0, 4.5, 1.0, 1.0, NUMPAD_GX;
    "Numpad3", "3", KeyCode::KEY_KP3, 2.0, 4.5, 1.0, 1.0, NUMPAD_GX;
    "NumpadEnter", "⏎", KeyCode::KEY_KPENTER, 3.0, 4.5, 1.0, 2.0, NUMPAD_GX;
    "Numpad0", "0", KeyCode::KEY_KP0, 0.0, 5.5, 2.0, 1.0, NUMPAD_GX;
    "NumpadDecimal", ".", KeyCode::KEY_KPDOT, 2.0, 5.5, 1.0, 1.0, NUMPAD_GX;
];

/// Find a key cap by its code identifier.
pub fn key(code: &str) -> Option<&'static KeyCap> {
    ALL_KEYS.iter().find(|cap| cap.code == code)
}

/// Find a key cap by the evdev scancode reported by the monitor.
pub fn key_by_evdev(scancode: u16) -> Option<&'static KeyCap> {
    ALL_KEYS.iter().find(|cap| cap.evdev == scancode)
}

/// Friendly display name for a key code (`keyName` in the export).
pub fn key_name(code: &str) -> String {
    const NAMES: &[(&str, &str)] = &[
        ("Backquote", "Backtick"),
        ("Minus", "Minus"),
        ("Equal", "Equal"),
        ("BracketLeft", "Left Bracket"),
        ("BracketRight", "Right Bracket"),
        ("Backslash", "Backslash"),
        ("Semicolon", "Semicolon"),
        ("Quote", "Quote"),
        ("Comma", "Comma"),
        ("Period", "Period"),
        ("Slash", "Slash"),
        ("CapsLock", "Caps Lock"),
        ("ShiftLeft", "Left Shift"),
        ("ShiftRight", "Right Shift"),
        ("ControlLeft", "Left Control"),
        ("ControlRight", "Right Control"),
        ("AltLeft", "Left Alt"),
        ("AltRight", "Right Alt"),
        ("MetaLeft", "Left Super"),
        ("MetaRight", "Right Super"),
        ("ContextMenu", "Menu"),
        ("Space", "Space"),
        ("Enter", "Enter"),
        ("Tab", "Tab"),
        ("Backspace", "Backspace"),
        ("Escape", "Escape"),
        ("ArrowUp", "Arrow Up"),
        ("ArrowDown", "Arrow Down"),
        ("ArrowLeft", "Arrow Left"),
        ("ArrowRight", "Arrow Right"),
        ("PrintScreen", "Print Screen"),
        ("ScrollLock", "Scroll Lock"),
        ("Pause", "Pause"),
        ("Insert", "Insert"),
        ("Delete", "Delete"),
        ("Home", "Home"),
        ("End", "End"),
        ("PageUp", "Page Up"),
        ("PageDown", "Page Down"),
        ("NumLock", "Num Lock"),
    ];
    if let Some((_, name)) = NAMES.iter().find(|(c, _)| *c == code) {
        return (*name).to_owned();
    }
    if let Some(rest) = code.strip_prefix("Key") {
        return rest.to_owned();
    }
    if let Some(rest) = code.strip_prefix("Digit") {
        return rest.to_owned();
    }
    if let Some(rest) = code.strip_prefix("Numpad") {
        return format!("Numpad {rest}");
    }
    code.to_owned()
}

/// Compact display form of an action, for key caps and pills.
pub fn short(action: &str) -> &str {
    const SHORT: &[(&str, &str)] = &[
        ("Escape", "Esc"),
        ("Control", "Ctrl"),
        ("Super", "Super"),
        ("Alt", "Alt"),
        ("Shift", "Shift"),
        ("Caps Lock", "Caps"),
        ("Hyper", "Hyper"),
        ("Home", "Home"),
        ("End", "End"),
        ("Page Up", "PgUp"),
        ("Page Down", "PgDn"),
        ("Left", "←"),
        ("Right", "→"),
        ("Up", "↑"),
        ("Down", "↓"),
        ("Backspace", "⌫"),
        ("Delete", "Del"),
        ("Tab", "Tab"),
        ("Enter", "⏎"),
        ("Play/Pause", "Play"),
        ("Next", "Next"),
        ("Previous", "Prev"),
        ("Mute", "Mute"),
        ("Volume Up", "Vol +"),
        ("Volume Down", "Vol −"),
        ("Brightness Up", "Bri +"),
        ("Brightness Down", "Bri −"),
        ("Disabled", "Off"),
        ("Print Screen", "PrtSc"),
        ("Menu", "Menu"),
        ("Insert", "Ins"),
        ("Minus", "-"),
        ("Equal", "="),
        ("Bracket Left", "["),
        ("Bracket Right", "]"),
        ("Backslash", "\\"),
        ("Semicolon", ";"),
        ("Quote", "'"),
        ("Comma", ","),
        ("Period", "."),
        ("Slash", "/"),
        ("Backtick", "`"),
        ("Space", "Space"),
        ("Right Control", "RCtrl"),
        ("Right Alt", "RAlt"),
        ("Right Super", "RSuper"),
    ];
    SHORT
        .iter()
        .find(|(name, _)| *name == action)
        .map_or(action, |(_, s)| s)
}

/// The searchable action catalog offered by the key editor.
pub const ACTION_GROUPS: &[(&str, &[&str])] = &[
    (
        "Modifiers",
        &[
            "Escape",
            "Control",
            "Right Control",
            "Super",
            "Right Super",
            "Alt",
            "Right Alt",
            "Shift",
            "Caps Lock",
            "Hyper",
        ],
    ),
    (
        "Navigation",
        &[
            "Home",
            "End",
            "Page Up",
            "Page Down",
            "Left",
            "Down",
            "Up",
            "Right",
            "Tab",
            "Enter",
            "Backspace",
            "Delete",
        ],
    ),
    (
        "Media",
        &[
            "Play/Pause",
            "Next",
            "Previous",
            "Mute",
            "Volume Up",
            "Volume Down",
            "Brightness Up",
            "Brightness Down",
        ],
    ),
    (
        "Letters",
        &[
            "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q",
            "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
        ],
    ),
    (
        "Numbers",
        &["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
    ),
    (
        "Function keys",
        &[
            "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
        ],
    ),
    (
        "Punctuation",
        &[
            "Minus",
            "Equal",
            "Bracket Left",
            "Bracket Right",
            "Backslash",
            "Semicolon",
            "Quote",
            "Comma",
            "Period",
            "Slash",
            "Backtick",
        ],
    ),
    (
        "Other",
        &["Space", "Print Screen", "Menu", "Insert", "Disabled"],
    ),
];

/// Category preselected in the editor for a given key (`autoGroup`).
pub fn auto_group(code: &str) -> &'static str {
    if code.starts_with("Key") {
        "Letters"
    } else if code.starts_with("Digit") || code.starts_with("Numpad") {
        "Numbers"
    } else if code.len() >= 2
        && code.starts_with('F')
        && code[1..].chars().all(|c| c.is_ascii_digit())
    {
        "Function keys"
    } else if [
        "Minus",
        "Equal",
        "BracketLeft",
        "BracketRight",
        "Backslash",
        "Semicolon",
        "Quote",
        "Comma",
        "Period",
        "Slash",
        "Backquote",
    ]
    .contains(&code)
    {
        "Punctuation"
    } else {
        "Modifiers"
    }
}

/// The navigation layer previewed while Caps Lock is held.
pub fn nav_layer(code: &str) -> Option<&'static str> {
    match code {
        "KeyH" => Some("←"),
        "KeyJ" => Some("↓"),
        "KeyK" => Some("↑"),
        "KeyL" => Some("→"),
        "KeyU" => Some("PgUp"),
        "KeyD" => Some("PgDn"),
        "KeyA" => Some("Home"),
        "KeyE" => Some("End"),
        "KeyY" => Some("⌫"),
        "KeyN" => Some("Del"),
        _ => None,
    }
}

/// What a remapped key does in one profile.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub tap: Option<String>,
    pub hold: Option<String>,
    /// Device scope id (`all` or a specific keyboard id).
    pub device: String,
    /// Two-way swap with the tap action.
    pub swap: bool,
}

/// Insertion-ordered key → mapping list, as the summary chips expect.
pub type Maps = Vec<(String, Mapping)>;

/// One side of a shortcut rule: held modifiers plus a key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    pub mods: Vec<String>,
    pub key: String,
}

impl Chord {
    pub fn is_empty(&self) -> bool {
        self.key.is_empty() && self.mods.is_empty()
    }
}

/// One shortcut rule inside a group.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rule {
    pub from: Chord,
    pub to: Chord,
    pub note: String,
}

/// A group of shortcut rules, optionally scoped to applications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub name: String,
    /// Application names this group is limited to; empty = all apps.
    pub apps: Vec<String>,
    pub enabled: bool,
    /// Match even while unrelated modifiers are held.
    pub any_mod: bool,
    pub rules: Vec<Rule>,
}

impl Group {
    pub fn scope_label(&self) -> String {
        if self.apps.is_empty() {
            "All applications".to_owned()
        } else {
            self.apps.join(", ")
        }
    }
}

/// A named remapping profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
}

fn mapping(tap: &str, hold: Option<&str>, device: &str, swap: bool) -> Mapping {
    Mapping {
        tap: Some(tap.to_owned()),
        hold: hold.map(str::to_owned),
        device: device.to_owned(),
        swap,
    }
}

fn entry(code: &str, mapping: Mapping) -> (String, Mapping) {
    (code.to_owned(), mapping)
}

/// The demo profiles shipped with the design prototype.
pub fn demo_profiles() -> Vec<(Profile, Maps)> {
    let profile = |id: &str, name: &str| Profile {
        id: id.to_owned(),
        name: name.to_owned(),
    };

    vec![
        (profile("default", "Default"), Vec::new()),
        (
            profile("laptop", "Laptop"),
            vec![
                entry(
                    "CapsLock",
                    mapping("Escape", Some("Control"), "builtin", false),
                ),
                entry("ContextMenu", mapping("Super", None, "all", false)),
                entry("F12", mapping("Play/Pause", None, "all", false)),
            ],
        ),
        (
            profile("mac", "Mac-style"),
            vec![
                entry("MetaLeft", mapping("Alt", None, "apple", false)),
                entry("AltLeft", mapping("Super", None, "apple", false)),
                entry("CapsLock", mapping("Control", None, "apple", false)),
            ],
        ),
        (
            profile("gaming", "Gaming"),
            vec![
                entry("MetaLeft", mapping("Disabled", None, "all", false)),
                entry("CapsLock", mapping("Disabled", None, "all", false)),
            ],
        ),
        (
            profile("cosmic", "Mac + Cosmic"),
            vec![
                entry("MetaLeft", mapping("Alt", None, "keychron", false)),
                entry("AltLeft", mapping("Right Control", None, "keychron", false)),
                entry(
                    "AltRight",
                    mapping("Right Control", None, "keychron", false),
                ),
                entry("MetaRight", mapping("Right Alt", None, "keychron", false)),
                entry("CapsLock", mapping("Super", None, "keychron", false)),
                entry("F1", mapping("Brightness Down", None, "keychron", true)),
                entry("F2", mapping("Brightness Up", None, "keychron", true)),
                entry("F7", mapping("Previous", None, "keychron", true)),
                entry("F8", mapping("Play/Pause", None, "keychron", true)),
                entry("F9", mapping("Next", None, "keychron", true)),
                entry("F10", mapping("Mute", None, "keychron", true)),
                entry("F11", mapping("Volume Down", None, "keychron", true)),
                entry("F12", mapping("Volume Up", None, "keychron", true)),
            ],
        ),
    ]
}

fn chord(mods: &[&str], key: &str) -> Chord {
    Chord {
        mods: mods.iter().map(|m| (*m).to_owned()).collect(),
        key: key.to_owned(),
    }
}

fn rule(from: Chord, to: Chord, note: &str) -> Rule {
    Rule {
        from,
        to,
        note: note.to_owned(),
    }
}

/// The demo shortcut groups for the `Mac + Cosmic` profile.
pub fn demo_groups() -> Vec<(String, Vec<Group>)> {
    let term_rules: Vec<Rule> = [
        ("T", "New tab"),
        ("N", "New window"),
        ("W", "Close tab"),
        ("Q", "Close window"),
        ("C", "Copy"),
        ("V", "Paste"),
        ("F", "Find"),
        ("G", "Find next"),
        ("H", "Find previous"),
        ("J", "Clear highlight"),
    ]
    .iter()
    .map(|(key, note)| {
        rule(
            chord(&["R Ctrl"], key),
            chord(&["Ctrl", "Shift"], key),
            note,
        )
    })
    .collect();

    let media_rules: Vec<Rule> = [
        "Volume Up",
        "Volume Down",
        "Mute",
        "Play/Pause",
        "Next",
        "Previous",
    ]
    .iter()
    .map(|key| rule(chord(&["Any"], key), chord(&[], key), ""))
    .collect();

    vec![(
        "cosmic".to_owned(),
        vec![
            Group {
                id: "g1".to_owned(),
                name: "Cosmic Desktop".to_owned(),
                apps: Vec::new(),
                enabled: true,
                any_mod: false,
                rules: vec![
                    rule(chord(&["R Ctrl"], "Space"), chord(&[], "Super"), "Launcher"),
                    rule(chord(&[], "F3"), chord(&["Super"], "W"), "Workspaces"),
                    rule(chord(&[], "F4"), chord(&["Super"], "A"), "Applications"),
                ],
            },
            Group {
                id: "g2".to_owned(),
                name: "Terminals".to_owned(),
                apps: vec!["Cosmic Terminal".to_owned()],
                enabled: true,
                any_mod: false,
                rules: term_rules,
            },
            Group {
                id: "g3".to_owned(),
                name: "Media keys ignore modifiers".to_owned(),
                apps: Vec::new(),
                enabled: true,
                any_mod: true,
                rules: media_rules,
            },
        ],
    )]
}

/// Demo device scopes from the prototype, used until real keyboards are
/// discovered (and to label the demo profiles' scopes).
pub const DEMO_DEVICES: &[(&str, &str, &str)] = &[
    ("all", "All keyboards", "Demo device choices"),
    ("builtin", "Built-in keyboard", "Demo laptop keyboard"),
    ("keychron", "Keychron K2 Pro", "Demo USB keyboard"),
    ("apple", "Apple Magic Keyboard", "Demo Bluetooth keyboard"),
];

/// The three onboarding steps.
pub const ONBOARDING: &[(&str, &str, &str, &str, &str)] = &[
    (
        "A",
        "Your keyboard, your rules",
        "Press a key to see it light up. Mappings you add are saved and written to your xremap configuration automatically.",
        "Continue with an example",
        "Waiting for input…",
    ),
    (
        "Caps",
        "Try the classic first",
        "For example, make Caps Lock behave like Escape. You can restore the original key whenever you want.",
        "Set Caps → Esc",
        "Step 2 of 3",
    ),
    (
        "✓",
        "Your first remap is ready",
        "Click a key, choose what it should do. Layers, tap-and-hold and per-app rules live in the same place when you want them.",
        "Open Keyloom",
        "Step 3 of 3",
    ),
];

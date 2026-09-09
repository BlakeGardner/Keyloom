//! UI data model ported from the design export's prototype script:
//! key geometry, the action catalog, and the built-in preset profiles,
//! plus the per-form-factor deck assembly.

use std::sync::LazyLock;

use evdev::KeyCode;
use serde::{Deserialize, Serialize};

use crate::keyboard;

/// One key unit in the deck, in logical pixels (`U` in the export).
pub const UNIT: f32 = 49.0;

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

/// Physical keys that appear only on some decks: the ISO 102nd key and
/// the compact boards' Fn key. Geometry here is their canonical spot;
/// deck assembly repositions copies as needed.
pub const EXTRA_KEYS: &[KeyCap] = caps![
    "IntlBackslash", "\\", KeyCode::KEY_102ND, 1.25, 4.5, 1.0, 1.0, 0.0;
    "Fn", "Fn", KeyCode::KEY_FN, 11.0, 5.5, 1.0, 1.0, 0.0;
];

/// Every physical key the app knows, across all decks and variants.
pub fn registry() -> impl Iterator<Item = &'static KeyCap> {
    ALL_KEYS.iter().chain(EXTRA_KEYS)
}

/// Find a key cap by its code identifier.
pub fn key(code: &str) -> Option<&'static KeyCap> {
    registry().find(|cap| cap.code == code)
}

/// Find a key cap by the evdev scancode reported by the monitor.
pub fn key_by_evdev(scancode: u16) -> Option<&'static KeyCap> {
    registry().find(|cap| cap.evdev == scancode)
}

/// The decks for every form factor and variant, assembled once.
/// Index: `form * 2 + iso`.
static DECKS: LazyLock<Vec<Vec<KeyCap>>> = LazyLock::new(|| {
    (0..keyboard::FORM_FACTORS.len())
        .flat_map(|form| [build_deck(form, false), build_deck(form, true)])
        .collect()
});

/// The key caps of one deck: a form factor in the ANSI or ISO variant.
pub fn deck(form: usize, iso: bool) -> &'static [KeyCap] {
    let index = form.min(keyboard::FORM_FACTORS.len() - 1) * 2 + usize::from(iso);
    &DECKS[index]
}

/// Rendered size of a deck in logical pixels, matching the export's
/// canvas insets (keys are drawn slightly smaller than their unit box).
pub fn deck_size(keys: &[KeyCap]) -> (f32, f32) {
    let width = keys
        .iter()
        .map(|cap| cap.gx + (cap.x + cap.w) * UNIT)
        .fold(0.0, f32::max);
    let height = keys
        .iter()
        .map(|cap| (cap.y + cap.h) * UNIT)
        .fold(0.0, f32::max);
    (width - 5.0, height - 5.5)
}

/// A registry cap repositioned for a specific deck.
fn place(code: &str, x: f32, y: f32, w: f32) -> KeyCap {
    let cap = key(code).expect("deck key must exist in the registry");
    KeyCap {
        x,
        y,
        w,
        h: 1.0,
        gx: 0.0,
        ..*cap
    }
}

/// Assemble one deck. The 100% and TKL decks reuse the design export's
/// exact geometry; the compact boards follow the classic assemblies
/// (right-hand column, ↑ carved out of right Shift, squeezed bottom row).
fn build_deck(form: usize, iso: bool) -> Vec<KeyCap> {
    let mut keys = match form {
        keyboard::FORM_TKL => ALL_KEYS
            .iter()
            .filter(|cap| cap.gx < NUMPAD_GX)
            .copied()
            .collect(),
        keyboard::FORM_SEVENTY_FIVE => {
            // Tight function row: Esc, F1–F12, PrtSc, Ins, Del.
            let mut keys = vec![place("Escape", 0.0, 0.0, 1.0)];
            for (i, f) in [
                "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
            ]
            .iter()
            .enumerate()
            {
                #[allow(clippy::cast_precision_loss)]
                keys.push(place(f, 1.0 + i as f32, 0.0, 1.0));
            }
            keys.push(place("PrintScreen", 13.0, 0.0, 1.0));
            keys.push(place("Insert", 14.0, 0.0, 1.0));
            keys.push(place("Delete", 15.0, 0.0, 1.0));
            keys.extend(compact_core(1.5, &["PageUp", "PageDown", "Home", "End"]));
            keys
        }
        keyboard::FORM_SIXTY_FIVE => {
            compact_core(0.0, &["Delete", "PageUp", "PageDown", "End"]).collect()
        }
        keyboard::FORM_SIXTY => ALL_KEYS
            .iter()
            .filter(|cap| cap.gx == 0.0 && cap.y >= 1.5)
            .map(|cap| KeyCap {
                y: cap.y - 1.5,
                ..*cap
            })
            .collect(),
        // 100% and anything out of range.
        _ => ALL_KEYS.to_vec(),
    };
    if iso {
        to_iso(&mut keys);
    }
    keys
}

/// The compact (65%/75%) assembly below the function row: the four core
/// rows plus a right-hand `column` key each, ↑ carved out of a shortened
/// right Shift, and the squeezed bottom row. `dy` shifts everything down
/// to leave room for a function row.
fn compact_core(dy: f32, column: &[&str; 4]) -> impl Iterator<Item = KeyCap> {
    // The core rows of the design deck (number row through bottom letter
    // row), shifted from their 100%-deck positions.
    let mut keys: Vec<KeyCap> = ALL_KEYS
        .iter()
        .filter(|cap| cap.gx == 0.0 && cap.y >= 1.5 && cap.y < 5.5)
        .map(|cap| KeyCap {
            y: cap.y - 1.5 + dy,
            ..*cap
        })
        .collect();

    // Shrink the right Shift to make room for the ↑ key.
    if let Some(shift) = keys.iter_mut().find(|cap| cap.code == "ShiftRight") {
        shift.w = 1.75;
    }
    keys.push(place("ArrowUp", 14.0, 3.0 + dy, 1.0));

    // The right-hand column.
    for (i, code) in column.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        keys.push(place(code, 15.0, i as f32 + dy, 1.0));
    }

    // Squeezed bottom row with the arrow cluster.
    let bottom = 4.0 + dy;
    keys.push(place("ControlLeft", 0.0, bottom, 1.25));
    keys.push(place("MetaLeft", 1.25, bottom, 1.25));
    keys.push(place("AltLeft", 2.5, bottom, 1.25));
    keys.push(place("Space", 3.75, bottom, 6.25));
    keys.push(place("AltRight", 10.0, bottom, 1.0));
    keys.push(place("Fn", 11.0, bottom, 1.0));
    keys.push(place("ControlRight", 12.0, bottom, 1.0));
    keys.push(place("ArrowLeft", 13.0, bottom, 1.0));
    keys.push(place("ArrowDown", 14.0, bottom, 1.0));
    keys.push(place("ArrowRight", 15.0, bottom, 1.0));

    keys.into_iter()
}

/// Convert an ANSI deck to the ISO assembly: two-segment tall Enter, the
/// `#` key beside it, a narrow left Shift plus the 102nd key, and AltGr.
/// The Enter segments share a scancode and highlight together.
fn to_iso(keys: &mut Vec<KeyCap>) {
    let Some(enter) = keys.iter().position(|cap| cap.code == "Enter") else {
        return;
    };
    let home_y = keys[enter].y;

    // Two-segment Enter replacing the ANSI bar Enter and the top-row \.
    keys[enter] = KeyCap {
        x: 13.5,
        y: home_y - 1.0,
        w: 1.5,
        h: 1.0,
        gx: 0.0,
        ..keys[enter]
    };
    let bottom_segment = KeyCap {
        label: "⏎",
        x: 13.75,
        y: home_y,
        w: 1.25,
        h: 1.0,
        gx: 0.0,
        ..keys[enter]
    };
    keys.push(bottom_segment);

    // The top-row \ moves next to Enter on the home row.
    if let Some(backslash) = keys.iter_mut().find(|cap| cap.code == "Backslash") {
        backslash.x = 12.75;
        backslash.y = home_y;
        backslash.w = 1.0;
    }

    // Narrow left Shift with the 102nd key beside it.
    if let Some(shift) = keys.iter_mut().find(|cap| cap.code == "ShiftLeft") {
        shift.w = 1.25;
    }
    keys.push(place("IntlBackslash", 1.25, home_y + 1.0, 1.0));

    // The right Alt becomes AltGr.
    if let Some(alt) = keys.iter_mut().find(|cap| cap.code == "AltRight") {
        alt.label = "AltGr";
    }
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
        ("IntlBackslash", "ISO Backslash"),
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
        ("Left Control", "LCtrl"),
        ("Left Alt", "LAlt"),
        ("Left Super", "LSuper"),
        ("Left Shift", "LShift"),
        ("Right Control", "RCtrl"),
        ("Right Alt", "RAlt"),
        ("Right Super", "RSuper"),
        ("Right Shift", "RShift"),
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
        ("Arrow Left", "←"),
        ("Arrow Right", "→"),
        ("Arrow Up", "↑"),
        ("Arrow Down", "↓"),
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
        ("Scroll Lock", "ScrLk"),
        ("Menu", "Menu"),
        ("Insert", "Ins"),
        ("Minus", "-"),
        ("Equal", "="),
        ("Bracket Left", "["),
        ("Bracket Right", "]"),
        ("Left Bracket", "["),
        ("Right Bracket", "]"),
        ("Backslash", "\\"),
        ("Semicolon", ";"),
        ("Quote", "'"),
        ("Comma", ","),
        ("Period", "."),
        ("Slash", "/"),
        ("Backtick", "`"),
        ("Space", "Space"),
        ("ISO Backslash", "ISO \\"),
        ("Num Lock", "NumLk"),
        ("Numpad Add", "Num +"),
        ("Numpad Subtract", "Num −"),
        ("Numpad Multiply", "Num *"),
        ("Numpad Divide", "Num /"),
        ("Numpad Enter", "Num ⏎"),
        ("Numpad Decimal", "Num ."),
    ];
    if let Some((_, s)) = SHORT.iter().find(|(name, _)| *name == action) {
        return s;
    }
    if let Some(rest) = action.strip_prefix("Numpad ") {
        // "Numpad 7" → "Num 7"; symbolic numpad keys are listed above.
        if rest.chars().all(|c| c.is_ascii_digit()) {
            const NUM: [&str; 10] = [
                "Num 0", "Num 1", "Num 2", "Num 3", "Num 4", "Num 5", "Num 6", "Num 7", "Num 8",
                "Num 9",
            ];
            if let Some(digit) = rest.parse::<usize>().ok().filter(|d| *d < 10) {
                return NUM[digit];
            }
        }
    }
    action
}

/// The searchable action catalog offered by the key editor. Names for
/// physical keys match [`key_name`] so the generator and the
/// self-mapping check treat them identically; older generic names
/// ("Control", "Left", …) remain accepted from stored mappings.
pub const ACTION_GROUPS: &[(&str, &[&str])] = &[
    (
        "Modifiers",
        &[
            "Escape",
            "Left Control",
            "Right Control",
            "Left Super",
            "Right Super",
            "Left Alt",
            "Right Alt",
            "Left Shift",
            "Right Shift",
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
            "Arrow Left",
            "Arrow Down",
            "Arrow Up",
            "Arrow Right",
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
        "Numpad",
        &[
            "Num Lock",
            "Numpad 0",
            "Numpad 1",
            "Numpad 2",
            "Numpad 3",
            "Numpad 4",
            "Numpad 5",
            "Numpad 6",
            "Numpad 7",
            "Numpad 8",
            "Numpad 9",
            "Numpad Add",
            "Numpad Subtract",
            "Numpad Multiply",
            "Numpad Divide",
            "Numpad Enter",
            "Numpad Decimal",
        ],
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
            "Left Bracket",
            "Right Bracket",
            "Backslash",
            "Semicolon",
            "Quote",
            "Comma",
            "Period",
            "Slash",
            "Backtick",
            "ISO Backslash",
        ],
    ),
    (
        "Other",
        &[
            "Space",
            "Print Screen",
            "Scroll Lock",
            "Pause",
            "Menu",
            "Insert",
            "Fn",
            "Disabled",
        ],
    ),
];

/// Category preselected in the editor for a given key (`autoGroup`).
pub fn auto_group(code: &str) -> &'static str {
    if code.starts_with("Key") {
        "Letters"
    } else if code.starts_with("Numpad") || code == "NumLock" {
        "Numpad"
    } else if code.starts_with("Digit") {
        "Numbers"
    } else if code == "Fn" {
        "Other"
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
        "IntlBackslash",
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

/// A built-in sample profile. Presets are read-only templates that live
/// outside the user's profile list; picking one in the profile switcher
/// creates an editable copy, so the preset itself never changes.
#[derive(Clone, Debug)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// One-line description shown in the profile switcher.
    pub blurb: &'static str,
    pub maps: Maps,
}

/// The sample presets shipped with the application. All mappings apply
/// to every keyboard: presets cannot know which devices exist.
pub fn presets() -> Vec<Preset> {
    let preset = |id, name, blurb, maps| Preset {
        id,
        name,
        blurb,
        maps,
    };

    vec![
        preset(
            "laptop",
            "Laptop",
            "Caps Lock taps Escape, holds Control",
            vec![
                entry(
                    "CapsLock",
                    mapping("Escape", Some("Left Control"), "all", false),
                ),
                entry("ContextMenu", mapping("Left Super", None, "all", false)),
                entry("F12", mapping("Play/Pause", None, "all", false)),
            ],
        ),
        preset(
            "mac",
            "Mac-style",
            "Command-style modifiers, Caps Lock as Control",
            vec![
                entry("MetaLeft", mapping("Left Alt", None, "all", false)),
                entry("AltLeft", mapping("Left Super", None, "all", false)),
                entry("CapsLock", mapping("Left Control", None, "all", false)),
            ],
        ),
        preset(
            "gaming",
            "Gaming",
            "Disables Super and Caps Lock",
            vec![
                entry("MetaLeft", mapping("Disabled", None, "all", false)),
                entry("CapsLock", mapping("Disabled", None, "all", false)),
            ],
        ),
        preset(
            "media",
            "Media F-row",
            "Function keys double as media keys (two-way)",
            vec![
                entry("F1", mapping("Brightness Down", None, "all", true)),
                entry("F2", mapping("Brightness Up", None, "all", true)),
                entry("F7", mapping("Previous", None, "all", true)),
                entry("F8", mapping("Play/Pause", None, "all", true)),
                entry("F9", mapping("Next", None, "all", true)),
                entry("F10", mapping("Mute", None, "all", true)),
                entry("F11", mapping("Volume Down", None, "all", true)),
                entry("F12", mapping("Volume Up", None, "all", true)),
            ],
        ),
    ]
}

/// The three onboarding steps.
pub const ONBOARDING: &[(&str, &str, &str, &str, &str)] = &[
    (
        "A",
        "Your keyboard, your rules",
        "Press a key to see it light up. Mappings you add are saved and applied automatically.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::{
        FORM_FACTORS, FORM_FULL, FORM_SEVENTY_FIVE, FORM_SIXTY, FORM_SIXTY_FIVE, FORM_TKL,
    };

    fn codes(form: usize, iso: bool) -> Vec<&'static str> {
        deck(form, iso).iter().map(|cap| cap.code).collect()
    }

    /// The 100% ANSI deck is the design export's geometry, untouched.
    #[test]
    fn full_ansi_deck_matches_the_design() {
        let full = deck(FORM_FULL, false);
        assert_eq!(full.len(), ALL_KEYS.len());
        assert_eq!(deck_size(full), (1099.0, 313.0));
    }

    /// Every deck key resolves through the registry, so mappings made on
    /// any deck work everywhere (physical identity is preserved).
    #[test]
    fn deck_keys_share_physical_identity() {
        for form in 0..FORM_FACTORS.len() {
            for iso in [false, true] {
                for cap in deck(form, iso) {
                    let registered = key(cap.code).expect("deck key registered");
                    assert_eq!(registered.evdev, cap.evdev, "{} diverged", cap.code);
                }
            }
        }
    }

    /// No two caps overlap on any deck (hand-positioned tables regress
    /// easily).
    #[test]
    fn deck_keys_do_not_overlap() {
        for form in 0..FORM_FACTORS.len() {
            for iso in [false, true] {
                let keys = deck(form, iso);
                for (i, a) in keys.iter().enumerate() {
                    for b in &keys[i + 1..] {
                        let ax = a.gx + a.x * UNIT;
                        let bx = b.gx + b.x * UNIT;
                        let overlap = ax < bx + b.w * UNIT - 0.5
                            && bx < ax + a.w * UNIT - 0.5
                            && a.y < b.y + b.h - 0.01
                            && b.y < a.y + a.h - 0.01;
                        assert!(
                            !overlap,
                            "form {form} iso {iso}: {} and {} overlap",
                            a.code, b.code
                        );
                    }
                }
            }
        }
    }

    /// Each size drops or gains the expected clusters.
    #[test]
    fn forms_have_their_clusters() {
        assert!(codes(FORM_FULL, false).contains(&"Numpad7"));
        assert!(!codes(FORM_TKL, false).contains(&"Numpad7"));
        assert!(codes(FORM_TKL, false).contains(&"Home"));
        assert!(codes(FORM_SEVENTY_FIVE, false).contains(&"F1"));
        assert!(codes(FORM_SEVENTY_FIVE, false).contains(&"ArrowUp"));
        assert!(!codes(FORM_SIXTY_FIVE, false).contains(&"F1"));
        assert!(codes(FORM_SIXTY_FIVE, false).contains(&"ArrowUp"));
        assert!(codes(FORM_SIXTY_FIVE, false).contains(&"Fn"));
        assert!(!codes(FORM_SIXTY, false).contains(&"ArrowUp"));
        assert!(!codes(FORM_SIXTY, false).contains(&"Escape"));
        assert!(codes(FORM_SIXTY, false).contains(&"KeyA"));
    }

    /// The ISO transform adds the 102nd key, splits Enter into two
    /// segments sharing a scancode, and relabels AltGr on every size.
    #[test]
    fn iso_decks_use_the_iso_assembly() {
        for form in 0..FORM_FACTORS.len() {
            let iso = deck(form, true);
            assert!(
                iso.iter().any(|cap| cap.code == "IntlBackslash"),
                "form {form} has the 102nd key"
            );
            let enters = iso.iter().filter(|cap| cap.code == "Enter").count();
            assert_eq!(enters, 2, "form {form} splits Enter into two segments");
            let alt = iso.iter().find(|cap| cap.code == "AltRight").unwrap();
            assert_eq!(alt.label, "AltGr");

            let ansi = deck(form, false);
            assert!(!ansi.iter().any(|cap| cap.code == "IntlBackslash"));
            assert_eq!(ansi.iter().filter(|cap| cap.code == "Enter").count(), 1);
        }
    }

    /// Physical identities are unique in the registry.
    #[test]
    fn registry_identities_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for cap in registry() {
            assert!(seen.insert(cap.code), "{} registered twice", cap.code);
        }
    }
}

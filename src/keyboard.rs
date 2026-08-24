//! Keyboard layout and form-factor definitions, plus system detection.
//!
//! Two independent axes describe the on-screen keyboard:
//!
//! * A [`Layout`] (language) pairs an ANSI or ISO core block with a
//!   character map assigning each character-key scancode its unshifted,
//!   shifted, and (optionally) AltGr output.
//! * A [`FormFactor`] (physical size, 100% down to 60%) assembles that
//!   core block into complete rows, adding the F-row, navigation keys,
//!   arrows, or numpad as the size allows.
//!
//! [`detect`] guesses the system language layout from configuration, and
//! [`form_for_keys`] guesses the physical size from a device's reported
//! keys.

use std::path::PathBuf;

/// A single key on the virtual keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A character-producing key, with its unshifted, shifted, and
    /// (optionally) AltGr output.
    Char {
        lower: char,
        upper: char,
        altgr: Option<char>,
    },
    Backspace,
    Tab,
    CapsLock,
    Enter,
    Shift,
    Ctrl,
    Super,
    Alt,
    AltGr,
    Space,
    /// A non-typing key (F-row, navigation, arrows, …) shown with a fixed
    /// label. Clicking it does nothing, but physical presses highlight it.
    Named(&'static str),
}

/// A key together with its evdev scancode and relative width.
///
/// Widths are expressed in quarter key-units (a standard key is 4),
/// and every geometry row below sums to 60 so keys line up across rows.
///
/// `code` is the Linux input-event scancode (`KEY_*` constant) for the
/// physical key this cap represents, letting evdev events map back to
/// the exact key — including distinguishing left from right modifiers.
#[derive(Clone, Copy, Debug)]
pub struct KeyDef {
    pub key: Key,
    pub code: u16,
    pub width: u16,
    /// Optional fixed cap label, overriding the default label for `key`.
    pub label: Option<&'static str>,
}

// Linux input-event scancodes (see linux/input-event-codes.h).
pub const KEY_1: u16 = 2;
pub const KEY_2: u16 = 3;
pub const KEY_3: u16 = 4;
pub const KEY_4: u16 = 5;
pub const KEY_5: u16 = 6;
pub const KEY_6: u16 = 7;
pub const KEY_7: u16 = 8;
pub const KEY_8: u16 = 9;
pub const KEY_9: u16 = 10;
pub const KEY_0: u16 = 11;
pub const KEY_MINUS: u16 = 12;
pub const KEY_EQUAL: u16 = 13;
pub const KEY_BACKSPACE: u16 = 14;
pub const KEY_TAB: u16 = 15;
pub const KEY_Q: u16 = 16;
pub const KEY_W: u16 = 17;
pub const KEY_E: u16 = 18;
pub const KEY_R: u16 = 19;
pub const KEY_T: u16 = 20;
pub const KEY_Y: u16 = 21;
pub const KEY_U: u16 = 22;
pub const KEY_I: u16 = 23;
pub const KEY_O: u16 = 24;
pub const KEY_P: u16 = 25;
pub const KEY_LEFTBRACE: u16 = 26;
pub const KEY_RIGHTBRACE: u16 = 27;
pub const KEY_ENTER: u16 = 28;
pub const KEY_LEFTCTRL: u16 = 29;
pub const KEY_A: u16 = 30;
pub const KEY_S: u16 = 31;
pub const KEY_D: u16 = 32;
pub const KEY_F: u16 = 33;
pub const KEY_G: u16 = 34;
pub const KEY_H: u16 = 35;
pub const KEY_J: u16 = 36;
pub const KEY_K: u16 = 37;
pub const KEY_L: u16 = 38;
pub const KEY_SEMICOLON: u16 = 39;
pub const KEY_APOSTROPHE: u16 = 40;
pub const KEY_GRAVE: u16 = 41;
pub const KEY_LEFTSHIFT: u16 = 42;
pub const KEY_BACKSLASH: u16 = 43;
pub const KEY_Z: u16 = 44;
pub const KEY_X: u16 = 45;
pub const KEY_C: u16 = 46;
pub const KEY_V: u16 = 47;
pub const KEY_B: u16 = 48;
pub const KEY_N: u16 = 49;
pub const KEY_M: u16 = 50;
pub const KEY_COMMA: u16 = 51;
pub const KEY_DOT: u16 = 52;
pub const KEY_SLASH: u16 = 53;
pub const KEY_RIGHTSHIFT: u16 = 54;
pub const KEY_LEFTALT: u16 = 56;
pub const KEY_SPACE: u16 = 57;
pub const KEY_CAPSLOCK: u16 = 58;
pub const KEY_102ND: u16 = 86;
pub const KEY_RIGHTCTRL: u16 = 97;
pub const KEY_RIGHTALT: u16 = 100;
pub const KEY_LEFTMETA: u16 = 125;
pub const KEY_RIGHTMETA: u16 = 126;
pub const KEY_ESC: u16 = 1;
pub const KEY_KPASTERISK: u16 = 55;
pub const KEY_F1: u16 = 59;
pub const KEY_F2: u16 = 60;
pub const KEY_F3: u16 = 61;
pub const KEY_F4: u16 = 62;
pub const KEY_F5: u16 = 63;
pub const KEY_F6: u16 = 64;
pub const KEY_F7: u16 = 65;
pub const KEY_F8: u16 = 66;
pub const KEY_F9: u16 = 67;
pub const KEY_F10: u16 = 68;
pub const KEY_NUMLOCK: u16 = 69;
pub const KEY_SCROLLLOCK: u16 = 70;
pub const KEY_KP7: u16 = 71;
pub const KEY_KP8: u16 = 72;
pub const KEY_KP9: u16 = 73;
pub const KEY_KPMINUS: u16 = 74;
pub const KEY_KP4: u16 = 75;
pub const KEY_KP5: u16 = 76;
pub const KEY_KP6: u16 = 77;
pub const KEY_KPPLUS: u16 = 78;
pub const KEY_KP1: u16 = 79;
pub const KEY_KP2: u16 = 80;
pub const KEY_KP3: u16 = 81;
pub const KEY_KP0: u16 = 82;
pub const KEY_KPDOT: u16 = 83;
pub const KEY_F11: u16 = 87;
pub const KEY_F12: u16 = 88;
pub const KEY_KPENTER: u16 = 96;
pub const KEY_KPSLASH: u16 = 98;
/// Print Screen.
pub const KEY_SYSRQ: u16 = 99;
pub const KEY_HOME: u16 = 102;
pub const KEY_UP: u16 = 103;
pub const KEY_PAGEUP: u16 = 104;
pub const KEY_LEFT: u16 = 105;
pub const KEY_RIGHT: u16 = 106;
pub const KEY_END: u16 = 107;
pub const KEY_DOWN: u16 = 108;
pub const KEY_PAGEDOWN: u16 = 109;
pub const KEY_INSERT: u16 = 110;
pub const KEY_DELETE: u16 = 111;
pub const KEY_PAUSE: u16 = 119;
/// The Fn key of compact boards (rarely actually reported over evdev).
pub const KEY_FN: u16 = 464;

/// One position in a physical geometry row.
#[derive(Clone, Copy, Debug)]
enum Slot {
    /// A character key: its symbols come from the layout's character map.
    Char { code: u16, width: u16 },
    /// A fixed key that is identical across layouts (modifiers, space, …).
    Fixed(KeyDef),
    /// A spacer between key clusters, in quarter key-units.
    Gap(u16),
}

/// A standard-width (1u) character key.
const fn c(code: u16) -> Slot {
    Slot::Char { code, width: 4 }
}

/// A character key with a custom width.
const fn cw(code: u16, width: u16) -> Slot {
    Slot::Char { code, width }
}

/// A fixed key with the default label.
const fn f(key: Key, code: u16, width: u16) -> Slot {
    Slot::Fixed(KeyDef {
        key,
        code,
        width,
        label: None,
    })
}

/// A fixed key with a custom label.
const fn fl(key: Key, code: u16, width: u16, label: &'static str) -> Slot {
    Slot::Fixed(KeyDef {
        key,
        code,
        width,
        label: Some(label),
    })
}

/// A display-only named key (F-row, navigation, …).
const fn named(name: &'static str, code: u16, width: u16) -> Slot {
    f(Key::Named(name), code, width)
}

/// A numpad character key: same output shifted or not, on every layout.
const fn kp(symbol: char, code: u16, width: u16) -> Slot {
    f(
        Key::Char {
            lower: symbol,
            upper: symbol,
            altgr: None,
        },
        code,
        width,
    )
}

/// ANSI (US-style) core block: wide `\` key, one-row Enter, and a plain
/// right Alt. Form factors assemble this block into complete boards.
static ANSI: &[&[Slot]] = &[
    &[
        c(KEY_GRAVE),
        c(KEY_1),
        c(KEY_2),
        c(KEY_3),
        c(KEY_4),
        c(KEY_5),
        c(KEY_6),
        c(KEY_7),
        c(KEY_8),
        c(KEY_9),
        c(KEY_0),
        c(KEY_MINUS),
        c(KEY_EQUAL),
        f(Key::Backspace, KEY_BACKSPACE, 8),
    ],
    &[
        f(Key::Tab, KEY_TAB, 6),
        c(KEY_Q),
        c(KEY_W),
        c(KEY_E),
        c(KEY_R),
        c(KEY_T),
        c(KEY_Y),
        c(KEY_U),
        c(KEY_I),
        c(KEY_O),
        c(KEY_P),
        c(KEY_LEFTBRACE),
        c(KEY_RIGHTBRACE),
        cw(KEY_BACKSLASH, 6),
    ],
    &[
        f(Key::CapsLock, KEY_CAPSLOCK, 7),
        c(KEY_A),
        c(KEY_S),
        c(KEY_D),
        c(KEY_F),
        c(KEY_G),
        c(KEY_H),
        c(KEY_J),
        c(KEY_K),
        c(KEY_L),
        c(KEY_SEMICOLON),
        c(KEY_APOSTROPHE),
        f(Key::Enter, KEY_ENTER, 9),
    ],
    &[
        f(Key::Shift, KEY_LEFTSHIFT, 9),
        c(KEY_Z),
        c(KEY_X),
        c(KEY_C),
        c(KEY_V),
        c(KEY_B),
        c(KEY_N),
        c(KEY_M),
        c(KEY_COMMA),
        c(KEY_DOT),
        c(KEY_SLASH),
        f(Key::Shift, KEY_RIGHTSHIFT, 11),
    ],
    &[
        f(Key::Ctrl, KEY_LEFTCTRL, 6),
        f(Key::Super, KEY_LEFTMETA, 5),
        f(Key::Alt, KEY_LEFTALT, 5),
        f(Key::Space, KEY_SPACE, 28),
        f(Key::Alt, KEY_RIGHTALT, 5),
        f(Key::Super, KEY_RIGHTMETA, 5),
        f(Key::Ctrl, KEY_RIGHTCTRL, 6),
    ],
];

/// ISO (European) core block: narrow left Shift plus the 102nd key,
/// an extra key next to Enter, AltGr, and a two-row Enter (drawn as two
/// stacked segments that map to the same scancode and highlight together).
static ISO: &[&[Slot]] = &[
    &[
        c(KEY_GRAVE),
        c(KEY_1),
        c(KEY_2),
        c(KEY_3),
        c(KEY_4),
        c(KEY_5),
        c(KEY_6),
        c(KEY_7),
        c(KEY_8),
        c(KEY_9),
        c(KEY_0),
        c(KEY_MINUS),
        c(KEY_EQUAL),
        f(Key::Backspace, KEY_BACKSPACE, 8),
    ],
    &[
        f(Key::Tab, KEY_TAB, 6),
        c(KEY_Q),
        c(KEY_W),
        c(KEY_E),
        c(KEY_R),
        c(KEY_T),
        c(KEY_Y),
        c(KEY_U),
        c(KEY_I),
        c(KEY_O),
        c(KEY_P),
        c(KEY_LEFTBRACE),
        c(KEY_RIGHTBRACE),
        f(Key::Enter, KEY_ENTER, 6),
    ],
    &[
        f(Key::CapsLock, KEY_CAPSLOCK, 7),
        c(KEY_A),
        c(KEY_S),
        c(KEY_D),
        c(KEY_F),
        c(KEY_G),
        c(KEY_H),
        c(KEY_J),
        c(KEY_K),
        c(KEY_L),
        c(KEY_SEMICOLON),
        c(KEY_APOSTROPHE),
        c(KEY_BACKSLASH),
        fl(Key::Enter, KEY_ENTER, 5, "⏎"),
    ],
    &[
        fl(Key::Shift, KEY_LEFTSHIFT, 5, "⇧"),
        c(KEY_102ND),
        c(KEY_Z),
        c(KEY_X),
        c(KEY_C),
        c(KEY_V),
        c(KEY_B),
        c(KEY_N),
        c(KEY_M),
        c(KEY_COMMA),
        c(KEY_DOT),
        c(KEY_SLASH),
        f(Key::Shift, KEY_RIGHTSHIFT, 11),
    ],
    &[
        f(Key::Ctrl, KEY_LEFTCTRL, 6),
        f(Key::Super, KEY_LEFTMETA, 5),
        f(Key::Alt, KEY_LEFTALT, 5),
        f(Key::Space, KEY_SPACE, 28),
        f(Key::AltGr, KEY_RIGHTALT, 5),
        f(Key::Super, KEY_RIGHTMETA, 5),
        f(Key::Ctrl, KEY_RIGHTCTRL, 6),
    ],
];

/// A materialized cap in a key row: a clickable key or a spacer.
#[derive(Clone, Copy, Debug)]
pub enum Cap {
    Key(KeyDef),
    Gap(u16),
}

#[derive(Clone, Copy, Debug)]
enum FormKind {
    Full,
    Tkl,
    SeventyFive,
    SixtyFive,
    Sixty,
}

/// A physical keyboard size, from full-size (100%) down to 60%.
pub struct FormFactor {
    /// Human-readable name shown in the size picker.
    pub name: &'static str,
    kind: FormKind,
}

/// All supported form factors, largest first.
pub static FORM_FACTORS: &[FormFactor] = &[
    FormFactor {
        name: "100% · Full-size",
        kind: FormKind::Full,
    },
    FormFactor {
        name: "80% · Tenkeyless",
        kind: FormKind::Tkl,
    },
    FormFactor {
        name: "75% · Compact",
        kind: FormKind::SeventyFive,
    },
    FormFactor {
        name: "65%",
        kind: FormKind::SixtyFive,
    },
    FormFactor {
        name: "60%",
        kind: FormKind::Sixty,
    },
];

/// Indexes into [`FORM_FACTORS`].
pub const FORM_FULL: usize = 0;
pub const FORM_TKL: usize = 1;
pub const FORM_SEVENTY_FIVE: usize = 2;
pub const FORM_SIXTY_FIVE: usize = 3;
pub const FORM_SIXTY: usize = 4;

/// The F-row function keys, F1 through F12.
static F_KEYS: [Slot; 12] = [
    named("F1", KEY_F1, 4),
    named("F2", KEY_F2, 4),
    named("F3", KEY_F3, 4),
    named("F4", KEY_F4, 4),
    named("F5", KEY_F5, 4),
    named("F6", KEY_F6, 4),
    named("F7", KEY_F7, 4),
    named("F8", KEY_F8, 4),
    named("F9", KEY_F9, 4),
    named("F10", KEY_F10, 4),
    named("F11", KEY_F11, 4),
    named("F12", KEY_F12, 4),
];

/// The shared core block in the ANSI or ISO flavor.
fn core_block(iso: bool) -> &'static [&'static [Slot]] {
    if iso { ISO } else { ANSI }
}

/// Bottom row of compact (65%/75%) boards: three 1u keys and the arrow
/// cluster squeezed to the right of a shorter space bar. Sums to 64.
fn compact_bottom(iso: bool) -> Vec<Slot> {
    let right_alt = if iso {
        f(Key::AltGr, KEY_RIGHTALT, 4)
    } else {
        f(Key::Alt, KEY_RIGHTALT, 4)
    };

    vec![
        f(Key::Ctrl, KEY_LEFTCTRL, 5),
        f(Key::Super, KEY_LEFTMETA, 5),
        f(Key::Alt, KEY_LEFTALT, 5),
        f(Key::Space, KEY_SPACE, 25),
        right_alt,
        named("Fn", KEY_FN, 4),
        f(Key::Ctrl, KEY_RIGHTCTRL, 4),
        named("←", KEY_LEFT, 4),
        named("↓", KEY_DOWN, 4),
        named("→", KEY_RIGHT, 4),
    ]
}

/// Compact (65%/75%) assembly: the core block plus one right-hand column,
/// an ↑ key carved out of the right Shift, and a compact bottom row.
/// Every row sums to 64 quarter-units (16u).
fn compact_rows(iso: bool, column: [Slot; 4]) -> Vec<Vec<Slot>> {
    let mut rows = Vec::with_capacity(5);

    for (i, core_row) in core_block(iso)[..4].iter().enumerate() {
        let mut row = core_row.to_vec();

        if i == 3 {
            // Shrink the right Shift to make room for the ↑ key.
            if let Some(Slot::Fixed(def)) = row.last_mut() {
                def.width = 7;
            }

            row.push(named("↑", KEY_UP, 4));
        }

        row.push(column[i]);
        rows.push(row);
    }

    rows.push(compact_bottom(iso));
    rows
}

/// The single-row 75% function row: everything packed with no gaps.
fn function_row_75() -> Vec<Slot> {
    let mut row = vec![named("Esc", KEY_ESC, 4)];
    row.extend(F_KEYS);
    row.extend([
        named("PrtSc", KEY_SYSRQ, 4),
        named("Ins", KEY_INSERT, 4),
        named("Del", KEY_DELETE, 4),
    ]);
    row
}

/// Tenkeyless assembly: F-row with cluster gaps, the core block, and the
/// nav/arrow island. Every row sums to 73 quarter-units (18.25u).
fn tkl_rows(iso: bool) -> Vec<Vec<Slot>> {
    let core = core_block(iso);

    let mut f_row = vec![named("Esc", KEY_ESC, 4), Slot::Gap(4)];
    for (i, key) in F_KEYS.iter().enumerate() {
        if i == 4 || i == 8 {
            f_row.push(Slot::Gap(2));
        }
        f_row.push(*key);
    }
    f_row.extend([
        Slot::Gap(1),
        named("PrtSc", KEY_SYSRQ, 4),
        named("ScrLk", KEY_SCROLLLOCK, 4),
        named("Pause", KEY_PAUSE, 4),
    ]);

    let mut rows = Vec::with_capacity(6);
    rows.push(f_row);

    let mut row = core[0].to_vec();
    row.extend([
        Slot::Gap(1),
        named("Ins", KEY_INSERT, 4),
        named("Home", KEY_HOME, 4),
        named("PgUp", KEY_PAGEUP, 4),
    ]);
    rows.push(row);

    let mut row = core[1].to_vec();
    row.extend([
        Slot::Gap(1),
        named("Del", KEY_DELETE, 4),
        named("End", KEY_END, 4),
        named("PgDn", KEY_PAGEDOWN, 4),
    ]);
    rows.push(row);

    let mut row = core[2].to_vec();
    row.push(Slot::Gap(13));
    rows.push(row);

    let mut row = core[3].to_vec();
    row.extend([Slot::Gap(5), named("↑", KEY_UP, 4), Slot::Gap(4)]);
    rows.push(row);

    let mut row = core[4].to_vec();
    row.extend([
        Slot::Gap(1),
        named("←", KEY_LEFT, 4),
        named("↓", KEY_DOWN, 4),
        named("→", KEY_RIGHT, 4),
    ]);
    rows.push(row);

    rows
}

/// Full-size assembly: tenkeyless plus the numpad. The double-height
/// numpad + and Enter are drawn as two stacked segments sharing a
/// scancode, like the ISO Enter. Every row sums to 90 quarter-units
/// (22.5u).
fn full_rows(iso: bool) -> Vec<Vec<Slot>> {
    let mut rows = tkl_rows(iso);

    rows[0].push(Slot::Gap(17));
    rows[1].extend([
        Slot::Gap(1),
        named("Num", KEY_NUMLOCK, 4),
        kp('/', KEY_KPSLASH, 4),
        kp('*', KEY_KPASTERISK, 4),
        kp('-', KEY_KPMINUS, 4),
    ]);
    rows[2].extend([
        Slot::Gap(1),
        kp('7', KEY_KP7, 4),
        kp('8', KEY_KP8, 4),
        kp('9', KEY_KP9, 4),
        kp('+', KEY_KPPLUS, 4),
    ]);
    rows[3].extend([
        Slot::Gap(1),
        kp('4', KEY_KP4, 4),
        kp('5', KEY_KP5, 4),
        kp('6', KEY_KP6, 4),
        kp('+', KEY_KPPLUS, 4),
    ]);
    rows[4].extend([
        Slot::Gap(1),
        kp('1', KEY_KP1, 4),
        kp('2', KEY_KP2, 4),
        kp('3', KEY_KP3, 4),
        fl(Key::Enter, KEY_KPENTER, 4, "⏎"),
    ]);
    rows[5].extend([
        Slot::Gap(1),
        kp('0', KEY_KP0, 8),
        kp('.', KEY_KPDOT, 4),
        fl(Key::Enter, KEY_KPENTER, 4, "⏎"),
    ]);

    rows
}

impl FormFactor {
    /// Assemble the slot rows for this size in the ANSI or ISO flavor.
    fn slot_rows(&self, iso: bool) -> Vec<Vec<Slot>> {
        match self.kind {
            FormKind::Sixty => core_block(iso).iter().map(|row| row.to_vec()).collect(),
            FormKind::SixtyFive => compact_rows(
                iso,
                [
                    named("Del", KEY_DELETE, 4),
                    named("PgUp", KEY_PAGEUP, 4),
                    named("PgDn", KEY_PAGEDOWN, 4),
                    named("End", KEY_END, 4),
                ],
            ),
            FormKind::SeventyFive => {
                let mut rows = vec![function_row_75()];
                rows.extend(compact_rows(
                    iso,
                    [
                        named("PgUp", KEY_PAGEUP, 4),
                        named("PgDn", KEY_PAGEDOWN, 4),
                        named("Home", KEY_HOME, 4),
                        named("End", KEY_END, 4),
                    ],
                ));
                rows
            }
            FormKind::Tkl => tkl_rows(iso),
            FormKind::Full => full_rows(iso),
        }
    }
}

/// Best-effort form factor guess from the keys a device reports.
///
/// Absence is meaningful — a device that doesn't report numpad scancodes
/// has no numpad — but presence is not: compact boards often report every
/// key their Fn layer can emit, which inflates the guess. Detection is a
/// starting point; the size picker always wins.
pub fn form_for_keys(numpad: bool, nav_cluster: bool, f_row: bool, arrows: bool) -> usize {
    if numpad {
        FORM_FULL
    } else if nav_cluster {
        FORM_TKL
    } else if f_row {
        FORM_SEVENTY_FIVE
    } else if arrows {
        FORM_SIXTY_FIVE
    } else {
        FORM_SIXTY
    }
}

/// Form factor hint from a device's marketing name.
///
/// Keyboards routinely encode their size (`NEO80`, `Q65`) or key count
/// (`GK61`, `K552-87`) in the product name, which is often more truthful
/// than the over-reported key capabilities.
pub fn form_for_name(name: &str) -> Option<usize> {
    name.split(|c: char| !c.is_ascii_digit())
        .find_map(|run| match run {
            "96" | "98" | "100" | "104" | "108" => Some(FORM_FULL),
            "80" | "87" | "88" => Some(FORM_TKL),
            "75" | "84" => Some(FORM_SEVENTY_FIVE),
            "65" | "66" | "67" | "68" => Some(FORM_SIXTY_FIVE),
            "60" | "61" => Some(FORM_SIXTY),
            _ => None,
        })
}

/// The unshifted, shifted, and optional AltGr output of a character key.
///
/// Dead keys (e.g. `^` on German or `´` on Spanish) are rendered as their
/// plain spacing character; composition is not simulated.
#[derive(Clone, Copy, Debug)]
struct CharDef {
    lower: char,
    upper: char,
    altgr: Option<char>,
}

const fn ch(code: u16, lower: char, upper: char) -> (u16, CharDef) {
    (
        code,
        CharDef {
            lower,
            upper,
            altgr: None,
        },
    )
}

const fn ag(code: u16, lower: char, upper: char, altgr: char) -> (u16, CharDef) {
    (
        code,
        CharDef {
            lower,
            upper,
            altgr: Some(altgr),
        },
    )
}

/// US QWERTY.
static US: &[(u16, CharDef)] = &[
    ch(KEY_GRAVE, '`', '~'),
    ch(KEY_1, '1', '!'),
    ch(KEY_2, '2', '@'),
    ch(KEY_3, '3', '#'),
    ch(KEY_4, '4', '$'),
    ch(KEY_5, '5', '%'),
    ch(KEY_6, '6', '^'),
    ch(KEY_7, '7', '&'),
    ch(KEY_8, '8', '*'),
    ch(KEY_9, '9', '('),
    ch(KEY_0, '0', ')'),
    ch(KEY_MINUS, '-', '_'),
    ch(KEY_EQUAL, '=', '+'),
    ch(KEY_Q, 'q', 'Q'),
    ch(KEY_W, 'w', 'W'),
    ch(KEY_E, 'e', 'E'),
    ch(KEY_R, 'r', 'R'),
    ch(KEY_T, 't', 'T'),
    ch(KEY_Y, 'y', 'Y'),
    ch(KEY_U, 'u', 'U'),
    ch(KEY_I, 'i', 'I'),
    ch(KEY_O, 'o', 'O'),
    ch(KEY_P, 'p', 'P'),
    ch(KEY_LEFTBRACE, '[', '{'),
    ch(KEY_RIGHTBRACE, ']', '}'),
    ch(KEY_BACKSLASH, '\\', '|'),
    ch(KEY_A, 'a', 'A'),
    ch(KEY_S, 's', 'S'),
    ch(KEY_D, 'd', 'D'),
    ch(KEY_F, 'f', 'F'),
    ch(KEY_G, 'g', 'G'),
    ch(KEY_H, 'h', 'H'),
    ch(KEY_J, 'j', 'J'),
    ch(KEY_K, 'k', 'K'),
    ch(KEY_L, 'l', 'L'),
    ch(KEY_SEMICOLON, ';', ':'),
    ch(KEY_APOSTROPHE, '\'', '"'),
    ch(KEY_Z, 'z', 'Z'),
    ch(KEY_X, 'x', 'X'),
    ch(KEY_C, 'c', 'C'),
    ch(KEY_V, 'v', 'V'),
    ch(KEY_B, 'b', 'B'),
    ch(KEY_N, 'n', 'N'),
    ch(KEY_M, 'm', 'M'),
    ch(KEY_COMMA, ',', '<'),
    ch(KEY_DOT, '.', '>'),
    ch(KEY_SLASH, '/', '?'),
];

/// UK QWERTY (xkb `gb`).
static UK: &[(u16, CharDef)] = &[
    ag(KEY_GRAVE, '`', '¬', '¦'),
    ch(KEY_1, '1', '!'),
    ch(KEY_2, '2', '"'),
    ch(KEY_3, '3', '£'),
    ag(KEY_4, '4', '$', '€'),
    ch(KEY_5, '5', '%'),
    ch(KEY_6, '6', '^'),
    ch(KEY_7, '7', '&'),
    ch(KEY_8, '8', '*'),
    ch(KEY_9, '9', '('),
    ch(KEY_0, '0', ')'),
    ch(KEY_MINUS, '-', '_'),
    ch(KEY_EQUAL, '=', '+'),
    ch(KEY_Q, 'q', 'Q'),
    ch(KEY_W, 'w', 'W'),
    ch(KEY_E, 'e', 'E'),
    ch(KEY_R, 'r', 'R'),
    ch(KEY_T, 't', 'T'),
    ch(KEY_Y, 'y', 'Y'),
    ch(KEY_U, 'u', 'U'),
    ch(KEY_I, 'i', 'I'),
    ch(KEY_O, 'o', 'O'),
    ch(KEY_P, 'p', 'P'),
    ch(KEY_LEFTBRACE, '[', '{'),
    ch(KEY_RIGHTBRACE, ']', '}'),
    ch(KEY_A, 'a', 'A'),
    ch(KEY_S, 's', 'S'),
    ch(KEY_D, 'd', 'D'),
    ch(KEY_F, 'f', 'F'),
    ch(KEY_G, 'g', 'G'),
    ch(KEY_H, 'h', 'H'),
    ch(KEY_J, 'j', 'J'),
    ch(KEY_K, 'k', 'K'),
    ch(KEY_L, 'l', 'L'),
    ch(KEY_SEMICOLON, ';', ':'),
    ch(KEY_APOSTROPHE, '\'', '@'),
    ch(KEY_BACKSLASH, '#', '~'),
    ch(KEY_102ND, '\\', '|'),
    ch(KEY_Z, 'z', 'Z'),
    ch(KEY_X, 'x', 'X'),
    ch(KEY_C, 'c', 'C'),
    ch(KEY_V, 'v', 'V'),
    ch(KEY_B, 'b', 'B'),
    ch(KEY_N, 'n', 'N'),
    ch(KEY_M, 'm', 'M'),
    ch(KEY_COMMA, ',', '<'),
    ch(KEY_DOT, '.', '>'),
    ch(KEY_SLASH, '/', '?'),
];

/// German QWERTZ (xkb `de`).
static DE: &[(u16, CharDef)] = &[
    ch(KEY_GRAVE, '^', '°'),
    ch(KEY_1, '1', '!'),
    ag(KEY_2, '2', '"', '²'),
    ag(KEY_3, '3', '§', '³'),
    ch(KEY_4, '4', '$'),
    ch(KEY_5, '5', '%'),
    ch(KEY_6, '6', '&'),
    ag(KEY_7, '7', '/', '{'),
    ag(KEY_8, '8', '(', '['),
    ag(KEY_9, '9', ')', ']'),
    ag(KEY_0, '0', '=', '}'),
    ag(KEY_MINUS, 'ß', '?', '\\'),
    ch(KEY_EQUAL, '´', '`'),
    ag(KEY_Q, 'q', 'Q', '@'),
    ch(KEY_W, 'w', 'W'),
    ag(KEY_E, 'e', 'E', '€'),
    ch(KEY_R, 'r', 'R'),
    ch(KEY_T, 't', 'T'),
    ch(KEY_Y, 'z', 'Z'),
    ch(KEY_U, 'u', 'U'),
    ch(KEY_I, 'i', 'I'),
    ch(KEY_O, 'o', 'O'),
    ch(KEY_P, 'p', 'P'),
    ch(KEY_LEFTBRACE, 'ü', 'Ü'),
    ag(KEY_RIGHTBRACE, '+', '*', '~'),
    ch(KEY_A, 'a', 'A'),
    ch(KEY_S, 's', 'S'),
    ch(KEY_D, 'd', 'D'),
    ch(KEY_F, 'f', 'F'),
    ch(KEY_G, 'g', 'G'),
    ch(KEY_H, 'h', 'H'),
    ch(KEY_J, 'j', 'J'),
    ch(KEY_K, 'k', 'K'),
    ch(KEY_L, 'l', 'L'),
    ch(KEY_SEMICOLON, 'ö', 'Ö'),
    ch(KEY_APOSTROPHE, 'ä', 'Ä'),
    ch(KEY_BACKSLASH, '#', '\''),
    ag(KEY_102ND, '<', '>', '|'),
    ch(KEY_Z, 'y', 'Y'),
    ch(KEY_X, 'x', 'X'),
    ch(KEY_C, 'c', 'C'),
    ch(KEY_V, 'v', 'V'),
    ch(KEY_B, 'b', 'B'),
    ch(KEY_N, 'n', 'N'),
    ag(KEY_M, 'm', 'M', 'µ'),
    ch(KEY_COMMA, ',', ';'),
    ch(KEY_DOT, '.', ':'),
    ch(KEY_SLASH, '-', '_'),
];

/// French AZERTY (xkb `fr`).
static FR: &[(u16, CharDef)] = &[
    ch(KEY_GRAVE, '²', '~'),
    ch(KEY_1, '&', '1'),
    ag(KEY_2, 'é', '2', '~'),
    ag(KEY_3, '"', '3', '#'),
    ag(KEY_4, '\'', '4', '{'),
    ag(KEY_5, '(', '5', '['),
    ag(KEY_6, '-', '6', '|'),
    ag(KEY_7, 'è', '7', '`'),
    ag(KEY_8, '_', '8', '\\'),
    ag(KEY_9, 'ç', '9', '^'),
    ag(KEY_0, 'à', '0', '@'),
    ag(KEY_MINUS, ')', '°', ']'),
    ag(KEY_EQUAL, '=', '+', '}'),
    ch(KEY_Q, 'a', 'A'),
    ch(KEY_W, 'z', 'Z'),
    ag(KEY_E, 'e', 'E', '€'),
    ch(KEY_R, 'r', 'R'),
    ch(KEY_T, 't', 'T'),
    ch(KEY_Y, 'y', 'Y'),
    ch(KEY_U, 'u', 'U'),
    ch(KEY_I, 'i', 'I'),
    ch(KEY_O, 'o', 'O'),
    ch(KEY_P, 'p', 'P'),
    ch(KEY_LEFTBRACE, '^', '¨'),
    ag(KEY_RIGHTBRACE, '$', '£', '¤'),
    ch(KEY_A, 'q', 'Q'),
    ch(KEY_S, 's', 'S'),
    ch(KEY_D, 'd', 'D'),
    ch(KEY_F, 'f', 'F'),
    ch(KEY_G, 'g', 'G'),
    ch(KEY_H, 'h', 'H'),
    ch(KEY_J, 'j', 'J'),
    ch(KEY_K, 'k', 'K'),
    ch(KEY_L, 'l', 'L'),
    ch(KEY_SEMICOLON, 'm', 'M'),
    ch(KEY_APOSTROPHE, 'ù', '%'),
    ch(KEY_BACKSLASH, '*', 'µ'),
    ch(KEY_102ND, '<', '>'),
    ch(KEY_Z, 'w', 'W'),
    ch(KEY_X, 'x', 'X'),
    ch(KEY_C, 'c', 'C'),
    ch(KEY_V, 'v', 'V'),
    ch(KEY_B, 'b', 'B'),
    ch(KEY_N, 'n', 'N'),
    ch(KEY_M, ',', '?'),
    ch(KEY_COMMA, ';', '.'),
    ch(KEY_DOT, ':', '/'),
    ch(KEY_SLASH, '!', '§'),
];

/// Spanish (xkb `es`).
static ES: &[(u16, CharDef)] = &[
    ag(KEY_GRAVE, 'º', 'ª', '\\'),
    ag(KEY_1, '1', '!', '|'),
    ag(KEY_2, '2', '"', '@'),
    ag(KEY_3, '3', '·', '#'),
    ag(KEY_4, '4', '$', '~'),
    ch(KEY_5, '5', '%'),
    ag(KEY_6, '6', '&', '¬'),
    ch(KEY_7, '7', '/'),
    ch(KEY_8, '8', '('),
    ch(KEY_9, '9', ')'),
    ch(KEY_0, '0', '='),
    ch(KEY_MINUS, '\'', '?'),
    ch(KEY_EQUAL, '¡', '¿'),
    ch(KEY_Q, 'q', 'Q'),
    ch(KEY_W, 'w', 'W'),
    ag(KEY_E, 'e', 'E', '€'),
    ch(KEY_R, 'r', 'R'),
    ch(KEY_T, 't', 'T'),
    ch(KEY_Y, 'y', 'Y'),
    ch(KEY_U, 'u', 'U'),
    ch(KEY_I, 'i', 'I'),
    ch(KEY_O, 'o', 'O'),
    ch(KEY_P, 'p', 'P'),
    ag(KEY_LEFTBRACE, '`', '^', '['),
    ag(KEY_RIGHTBRACE, '+', '*', ']'),
    ch(KEY_A, 'a', 'A'),
    ch(KEY_S, 's', 'S'),
    ch(KEY_D, 'd', 'D'),
    ch(KEY_F, 'f', 'F'),
    ch(KEY_G, 'g', 'G'),
    ch(KEY_H, 'h', 'H'),
    ch(KEY_J, 'j', 'J'),
    ch(KEY_K, 'k', 'K'),
    ch(KEY_L, 'l', 'L'),
    ch(KEY_SEMICOLON, 'ñ', 'Ñ'),
    ag(KEY_APOSTROPHE, '´', '¨', '{'),
    ag(KEY_BACKSLASH, 'ç', 'Ç', '}'),
    ch(KEY_102ND, '<', '>'),
    ch(KEY_Z, 'z', 'Z'),
    ch(KEY_X, 'x', 'X'),
    ch(KEY_C, 'c', 'C'),
    ch(KEY_V, 'v', 'V'),
    ch(KEY_B, 'b', 'B'),
    ch(KEY_N, 'n', 'N'),
    ch(KEY_M, 'm', 'M'),
    ch(KEY_COMMA, ',', ';'),
    ch(KEY_DOT, '.', ':'),
    ch(KEY_SLASH, '-', '_'),
];

/// US Dvorak (xkb `us(dvorak)`).
static DVORAK: &[(u16, CharDef)] = &[
    ch(KEY_GRAVE, '`', '~'),
    ch(KEY_1, '1', '!'),
    ch(KEY_2, '2', '@'),
    ch(KEY_3, '3', '#'),
    ch(KEY_4, '4', '$'),
    ch(KEY_5, '5', '%'),
    ch(KEY_6, '6', '^'),
    ch(KEY_7, '7', '&'),
    ch(KEY_8, '8', '*'),
    ch(KEY_9, '9', '('),
    ch(KEY_0, '0', ')'),
    ch(KEY_MINUS, '[', '{'),
    ch(KEY_EQUAL, ']', '}'),
    ch(KEY_Q, '\'', '"'),
    ch(KEY_W, ',', '<'),
    ch(KEY_E, '.', '>'),
    ch(KEY_R, 'p', 'P'),
    ch(KEY_T, 'y', 'Y'),
    ch(KEY_Y, 'f', 'F'),
    ch(KEY_U, 'g', 'G'),
    ch(KEY_I, 'c', 'C'),
    ch(KEY_O, 'r', 'R'),
    ch(KEY_P, 'l', 'L'),
    ch(KEY_LEFTBRACE, '/', '?'),
    ch(KEY_RIGHTBRACE, '=', '+'),
    ch(KEY_BACKSLASH, '\\', '|'),
    ch(KEY_A, 'a', 'A'),
    ch(KEY_S, 'o', 'O'),
    ch(KEY_D, 'e', 'E'),
    ch(KEY_F, 'u', 'U'),
    ch(KEY_G, 'i', 'I'),
    ch(KEY_H, 'd', 'D'),
    ch(KEY_J, 'h', 'H'),
    ch(KEY_K, 't', 'T'),
    ch(KEY_L, 'n', 'N'),
    ch(KEY_SEMICOLON, 's', 'S'),
    ch(KEY_APOSTROPHE, '-', '_'),
    ch(KEY_Z, ';', ':'),
    ch(KEY_X, 'q', 'Q'),
    ch(KEY_C, 'j', 'J'),
    ch(KEY_V, 'k', 'K'),
    ch(KEY_B, 'x', 'X'),
    ch(KEY_N, 'b', 'B'),
    ch(KEY_M, 'm', 'M'),
    ch(KEY_COMMA, 'w', 'W'),
    ch(KEY_DOT, 'v', 'V'),
    ch(KEY_SLASH, 'z', 'Z'),
];

/// A keyboard layout: a language's character map plus its core-block flavor.
pub struct Layout {
    /// Human-readable name shown in the layout picker.
    pub name: &'static str,
    /// The XKB (layout, variant) pair this corresponds to, for detection.
    xkb: (&'static str, &'static str),
    /// Whether this layout uses the ISO core block (102nd key, tall Enter,
    /// AltGr) rather than ANSI.
    iso: bool,
    chars: &'static [(u16, CharDef)],
}

/// All supported layouts. The first entry is the fallback default.
pub static LAYOUTS: &[Layout] = &[
    Layout {
        name: "English (US)",
        xkb: ("us", ""),
        iso: false,
        chars: US,
    },
    Layout {
        name: "English (UK)",
        xkb: ("gb", ""),
        iso: true,
        chars: UK,
    },
    Layout {
        name: "German (QWERTZ)",
        xkb: ("de", ""),
        iso: true,
        chars: DE,
    },
    Layout {
        name: "French (AZERTY)",
        xkb: ("fr", ""),
        iso: true,
        chars: FR,
    },
    Layout {
        name: "Spanish",
        xkb: ("es", ""),
        iso: true,
        chars: ES,
    },
    Layout {
        name: "English (Dvorak)",
        xkb: ("us", "dvorak"),
        iso: false,
        chars: DVORAK,
    },
];

/// Sentinel for geometry slots missing from a character map; showing up in
/// the UI (or tests) indicates a bug in the layout data.
const MISSING: CharDef = CharDef {
    lower: '�',
    upper: '�',
    altgr: None,
};

impl Layout {
    fn char_def(&self, code: u16) -> CharDef {
        self.chars
            .iter()
            .find(|(c, _)| *c == code)
            .map_or(MISSING, |(_, def)| *def)
    }

    /// Materialize the key rows for this layout on the given form factor.
    pub fn rows(&self, form: &FormFactor) -> Vec<Vec<Cap>> {
        form.slot_rows(self.iso)
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|slot| match slot {
                        Slot::Gap(width) => Cap::Gap(width),
                        Slot::Fixed(def) => Cap::Key(def),
                        Slot::Char { code, width } => {
                            let CharDef {
                                lower,
                                upper,
                                altgr,
                            } = self.char_def(code);

                            Cap::Key(KeyDef {
                                key: Key::Char {
                                    lower,
                                    upper,
                                    altgr,
                                },
                                code,
                                width,
                                label: None,
                            })
                        }
                    })
                    .collect()
            })
            .collect()
    }
}

/// Best-effort detection of the system keyboard layout.
///
/// Sources, in order: the COSMIC compositor configuration, the
/// `XKB_DEFAULT_LAYOUT`/`XKB_DEFAULT_VARIANT` environment, and
/// `/etc/default/keyboard`. Returns an index into [`LAYOUTS`], or `None`
/// when nothing (or an unsupported layout) is configured.
pub fn detect() -> Option<usize> {
    let (layout, variant) = cosmic_config_xkb()
        .or_else(env_xkb)
        .or_else(etc_default_xkb)?;

    match_layout(&layout, &variant)
}

/// Map an XKB layout/variant pair (possibly comma-separated lists, as
/// configured) to an entry in [`LAYOUTS`]. Unknown variants fall back to
/// the base layout.
fn match_layout(layout: &str, variant: &str) -> Option<usize> {
    let layout = layout.split(',').next().unwrap_or_default().trim();
    let variant = variant.split(',').next().unwrap_or_default().trim();

    let position = |l: &str, v: &str| LAYOUTS.iter().position(|def| def.xkb == (l, v));

    position(layout, variant).or_else(|| position(layout, ""))
}

/// The layout configured for the COSMIC compositor, if any.
fn cosmic_config_xkb() -> Option<(String, String)> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

    let path = base.join("cosmic/com.system76.CosmicComp/v1/xkb_config");
    parse_cosmic_xkb(&std::fs::read_to_string(path).ok()?)
}

fn parse_cosmic_xkb(text: &str) -> Option<(String, String)> {
    let layout = ron_str_field(text, "layout").filter(|layout| !layout.trim().is_empty())?;
    let variant = ron_str_field(text, "variant").unwrap_or_default();
    Some((layout, variant))
}

/// Extract a `name: "value"` string field from a RON document, without
/// pulling in a full RON parser.
fn ron_str_field(text: &str, name: &str) -> Option<String> {
    let mut rest = text;

    while let Some(pos) = rest.find(name) {
        let preceded = rest[..pos].chars().next_back();
        let candidate = &rest[pos + name.len()..];
        rest = candidate;

        // Reject matches inside longer identifiers (e.g. `my_layout`).
        if preceded.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }

        let Some(after_colon) = candidate.trim_start().strip_prefix(':') else {
            continue;
        };

        // Non-string values (e.g. `options: None`) are not ours to parse.
        let Some(value) = after_colon.trim_start().strip_prefix('"') else {
            continue;
        };

        return value.find('"').map(|end| value[..end].to_owned());
    }

    None
}

/// The layout from the `XKB_DEFAULT_*` environment (honored by wlroots
/// compositors and others), if set.
fn env_xkb() -> Option<(String, String)> {
    let layout = std::env::var("XKB_DEFAULT_LAYOUT")
        .ok()
        .filter(|layout| !layout.trim().is_empty())?;
    let variant = std::env::var("XKB_DEFAULT_VARIANT").unwrap_or_default();
    Some((layout, variant))
}

/// The system-wide layout from `/etc/default/keyboard`, if present.
fn etc_default_xkb() -> Option<(String, String)> {
    parse_etc_default(&std::fs::read_to_string("/etc/default/keyboard").ok()?)
}

fn parse_etc_default(text: &str) -> Option<(String, String)> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let value = line.trim().strip_prefix(name)?.trim_start().strip_prefix('=')?;
            Some(value.trim().trim_matches(|c| c == '"' || c == '\'').to_owned())
        })
    };

    let layout = field("XKBLAYOUT").filter(|layout| !layout.trim().is_empty())?;
    Some((layout, field("XKBVARIANT").unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap_width(cap: &Cap) -> u16 {
        match cap {
            Cap::Key(def) => def.width,
            Cap::Gap(width) => *width,
        }
    }

    /// Every row of a form factor sums to that size's width, so rows align.
    #[test]
    fn rows_are_aligned() {
        let widths = [90, 73, 64, 64, 60];

        for (form, expected) in FORM_FACTORS.iter().zip(widths) {
            for layout in LAYOUTS {
                for (i, row) in layout.rows(form).iter().enumerate() {
                    let total: u16 = row.iter().map(cap_width).sum();
                    assert_eq!(total, expected, "{} {} row {i}", form.name, layout.name);
                }
            }
        }
    }

    /// The `FORM_*` index constants agree with the [`FORM_FACTORS`] order.
    #[test]
    fn form_indices_match() {
        assert_eq!(FORM_FACTORS.len(), 5);
        assert!(FORM_FACTORS[FORM_FULL].name.starts_with("100%"));
        assert!(FORM_FACTORS[FORM_TKL].name.starts_with("80%"));
        assert!(FORM_FACTORS[FORM_SEVENTY_FIVE].name.starts_with("75%"));
        assert!(FORM_FACTORS[FORM_SIXTY_FIVE].name.starts_with("65%"));
        assert!(FORM_FACTORS[FORM_SIXTY].name.starts_with("60%"));
    }

    /// Every character slot in every assembled board has an entry in the
    /// layout's character map.
    #[test]
    fn charmaps_are_complete() {
        for layout in LAYOUTS {
            for form in FORM_FACTORS {
                for cap in layout.rows(form).iter().flatten() {
                    if let Cap::Key(def) = cap
                        && let Key::Char { lower, .. } = def.key
                    {
                        assert_ne!(
                            lower, '�',
                            "layout {} lacks chars for scancode {} on {}",
                            layout.name, def.code, form.name
                        );
                    }
                }
            }
        }
    }

    /// Scancodes are unique within a board, except two-segment keys
    /// (ISO Enter, numpad + and numpad Enter).
    #[test]
    fn scancodes_are_unique() {
        for layout in LAYOUTS {
            for form in FORM_FACTORS {
                let mut counts = std::collections::HashMap::new();

                for cap in layout.rows(form).iter().flatten() {
                    if let Cap::Key(def) = cap {
                        *counts.entry(def.code).or_insert(0u32) += 1;
                    }
                }

                for (code, count) in counts {
                    let limit = if [KEY_ENTER, KEY_KPPLUS, KEY_KPENTER].contains(&code) {
                        2
                    } else {
                        1
                    };
                    assert!(
                        count <= limit,
                        "{} on {}: scancode {code} appears {count} times",
                        layout.name,
                        form.name
                    );
                }
            }
        }
    }

    /// The capability cascade maps to the expected sizes.
    #[test]
    fn guesses_form_factors() {
        assert_eq!(form_for_keys(true, true, true, true), FORM_FULL);
        assert_eq!(form_for_keys(false, true, true, true), FORM_TKL);
        assert_eq!(form_for_keys(false, false, true, true), FORM_SEVENTY_FIVE);
        assert_eq!(form_for_keys(false, false, false, true), FORM_SIXTY_FIVE);
        assert_eq!(form_for_keys(false, false, false, false), FORM_SIXTY);
    }

    /// Device names hint at sizes; unrelated digits don't.
    #[test]
    fn guesses_forms_from_names() {
        assert_eq!(form_for_name("@HFD NEO80 Keyboard"), Some(FORM_TKL));
        assert_eq!(form_for_name("Keychron Q65"), Some(FORM_SIXTY_FIVE));
        assert_eq!(form_for_name("SKYLOONG GK61"), Some(FORM_SIXTY));
        assert_eq!(form_for_name("Redragon K552-87"), Some(FORM_TKL));
        assert_eq!(form_for_name("Corsair K100"), Some(FORM_FULL));
        assert_eq!(form_for_name("Epomaker TH84"), Some(FORM_SEVENTY_FIVE));
        assert_eq!(form_for_name("TESmart DKS202-P24"), None);
        assert_eq!(form_for_name("Logitech MX Keys"), None);
        assert_eq!(form_for_name(""), None);
    }

    #[test]
    fn matches_xkb_names() {
        let index_of = |name: &str| LAYOUTS.iter().position(|l| l.name == name);

        assert_eq!(match_layout("us", ""), index_of("English (US)"));
        assert_eq!(match_layout("us", "dvorak"), index_of("English (Dvorak)"));
        // Unsupported variants fall back to the base layout.
        assert_eq!(match_layout("us", "intl"), index_of("English (US)"));
        assert_eq!(match_layout("gb", ""), index_of("English (UK)"));
        assert_eq!(match_layout("de", "nodeadkeys"), index_of("German (QWERTZ)"));
        // Multiple configured layouts: the first wins.
        assert_eq!(match_layout("fr,us", ""), index_of("French (AZERTY)"));
        assert_eq!(match_layout("es", ""), index_of("Spanish"));
        assert_eq!(match_layout("xx", ""), None);
    }

    #[test]
    fn parses_cosmic_config() {
        let text = r#"(
    rules: "",
    model: "",
    layout: "de,us",
    variant: "nodeadkeys",
    options: None,
    repeat_delay: 400,
    repeat_rate: 45,
)"#;

        assert_eq!(
            parse_cosmic_xkb(text),
            Some(("de,us".to_owned(), "nodeadkeys".to_owned()))
        );

        // An unset layout is no detection at all.
        assert_eq!(parse_cosmic_xkb(r#"(layout: "", variant: "")"#), None);
    }

    #[test]
    fn parses_etc_default_keyboard() {
        let text = "# KEYBOARD CONFIGURATION FILE\nXKBMODEL=\"pc105\"\nXKBLAYOUT=\"gb\"\nXKBVARIANT=\"\"\nBACKSPACE=\"guess\"\n";
        assert_eq!(parse_etc_default(text), Some(("gb".to_owned(), String::new())));

        assert_eq!(
            parse_etc_default("XKBLAYOUT=us\nXKBVARIANT=dvorak\n"),
            Some(("us".to_owned(), "dvorak".to_owned()))
        );

        assert_eq!(parse_etc_default("XKBMODEL=pc105\n"), None);
    }
}

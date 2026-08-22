//! Static definition of the on-screen keyboard layout.

/// A single key on the virtual keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A character-producing key, with its unshifted and shifted output.
    Char { lower: char, upper: char },
    Backspace,
    Tab,
    CapsLock,
    Enter,
    Shift,
    Ctrl,
    Super,
    Alt,
    Space,
}

/// A key together with its evdev scancode and relative width.
///
/// Widths are expressed in quarter key-units (a standard key is 4),
/// and every row below sums to 60 so keys line up across rows.
///
/// `code` is the Linux input-event scancode (`KEY_*` constant) for the
/// physical key this cap represents, letting evdev events map back to
/// the exact key — including distinguishing left from right modifiers.
#[derive(Clone, Copy, Debug)]
pub struct KeyDef {
    pub key: Key,
    pub code: u16,
    pub width: u16,
}

const fn ch(lower: char, upper: char, code: u16) -> KeyDef {
    KeyDef {
        key: Key::Char { lower, upper },
        code,
        width: 4,
    }
}

const fn key(key: Key, code: u16, width: u16) -> KeyDef {
    KeyDef { key, code, width }
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
pub const KEY_RIGHTCTRL: u16 = 97;
pub const KEY_RIGHTALT: u16 = 100;
pub const KEY_LEFTMETA: u16 = 125;
pub const KEY_RIGHTMETA: u16 = 126;

/// ANSI US QWERTY layout, one slice per keyboard row.
pub static ROWS: &[&[KeyDef]] = &[
    &[
        ch('`', '~', KEY_GRAVE),
        ch('1', '!', KEY_1),
        ch('2', '@', KEY_2),
        ch('3', '#', KEY_3),
        ch('4', '$', KEY_4),
        ch('5', '%', KEY_5),
        ch('6', '^', KEY_6),
        ch('7', '&', KEY_7),
        ch('8', '*', KEY_8),
        ch('9', '(', KEY_9),
        ch('0', ')', KEY_0),
        ch('-', '_', KEY_MINUS),
        ch('=', '+', KEY_EQUAL),
        key(Key::Backspace, KEY_BACKSPACE, 8),
    ],
    &[
        key(Key::Tab, KEY_TAB, 6),
        ch('q', 'Q', KEY_Q),
        ch('w', 'W', KEY_W),
        ch('e', 'E', KEY_E),
        ch('r', 'R', KEY_R),
        ch('t', 'T', KEY_T),
        ch('y', 'Y', KEY_Y),
        ch('u', 'U', KEY_U),
        ch('i', 'I', KEY_I),
        ch('o', 'O', KEY_O),
        ch('p', 'P', KEY_P),
        ch('[', '{', KEY_LEFTBRACE),
        ch(']', '}', KEY_RIGHTBRACE),
        key(Key::Char { lower: '\\', upper: '|' }, KEY_BACKSLASH, 6),
    ],
    &[
        key(Key::CapsLock, KEY_CAPSLOCK, 7),
        ch('a', 'A', KEY_A),
        ch('s', 'S', KEY_S),
        ch('d', 'D', KEY_D),
        ch('f', 'F', KEY_F),
        ch('g', 'G', KEY_G),
        ch('h', 'H', KEY_H),
        ch('j', 'J', KEY_J),
        ch('k', 'K', KEY_K),
        ch('l', 'L', KEY_L),
        ch(';', ':', KEY_SEMICOLON),
        ch('\'', '"', KEY_APOSTROPHE),
        key(Key::Enter, KEY_ENTER, 9),
    ],
    &[
        key(Key::Shift, KEY_LEFTSHIFT, 9),
        ch('z', 'Z', KEY_Z),
        ch('x', 'X', KEY_X),
        ch('c', 'C', KEY_C),
        ch('v', 'V', KEY_V),
        ch('b', 'B', KEY_B),
        ch('n', 'N', KEY_N),
        ch('m', 'M', KEY_M),
        ch(',', '<', KEY_COMMA),
        ch('.', '>', KEY_DOT),
        ch('/', '?', KEY_SLASH),
        key(Key::Shift, KEY_RIGHTSHIFT, 11),
    ],
    &[
        key(Key::Ctrl, KEY_LEFTCTRL, 6),
        key(Key::Super, KEY_LEFTMETA, 5),
        key(Key::Alt, KEY_LEFTALT, 5),
        key(Key::Space, KEY_SPACE, 28),
        key(Key::Alt, KEY_RIGHTALT, 5),
        key(Key::Super, KEY_RIGHTMETA, 5),
        key(Key::Ctrl, KEY_RIGHTCTRL, 6),
    ],
];

/// Look up the layout key for an evdev scancode.
pub fn key_for_code(code: u16) -> Option<Key> {
    ROWS.iter()
        .flat_map(|row| row.iter())
        .find(|def| def.code == code)
        .map(|def| def.key)
}

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

/// A key together with its relative width.
///
/// Widths are expressed in quarter key-units (a standard key is 4),
/// and every row below sums to 60 so keys line up across rows.
#[derive(Clone, Copy, Debug)]
pub struct KeyDef {
    pub key: Key,
    pub width: u16,
}

const fn ch(lower: char, upper: char) -> KeyDef {
    KeyDef {
        key: Key::Char { lower, upper },
        width: 4,
    }
}

const fn key(key: Key, width: u16) -> KeyDef {
    KeyDef { key, width }
}

/// ANSI US QWERTY layout, one slice per keyboard row.
pub static ROWS: &[&[KeyDef]] = &[
    &[
        ch('`', '~'),
        ch('1', '!'),
        ch('2', '@'),
        ch('3', '#'),
        ch('4', '$'),
        ch('5', '%'),
        ch('6', '^'),
        ch('7', '&'),
        ch('8', '*'),
        ch('9', '('),
        ch('0', ')'),
        ch('-', '_'),
        ch('=', '+'),
        key(Key::Backspace, 8),
    ],
    &[
        key(Key::Tab, 6),
        ch('q', 'Q'),
        ch('w', 'W'),
        ch('e', 'E'),
        ch('r', 'R'),
        ch('t', 'T'),
        ch('y', 'Y'),
        ch('u', 'U'),
        ch('i', 'I'),
        ch('o', 'O'),
        ch('p', 'P'),
        ch('[', '{'),
        ch(']', '}'),
        key(Key::Char { lower: '\\', upper: '|' }, 6),
    ],
    &[
        key(Key::CapsLock, 7),
        ch('a', 'A'),
        ch('s', 'S'),
        ch('d', 'D'),
        ch('f', 'F'),
        ch('g', 'G'),
        ch('h', 'H'),
        ch('j', 'J'),
        ch('k', 'K'),
        ch('l', 'L'),
        ch(';', ':'),
        ch('\'', '"'),
        key(Key::Enter, 9),
    ],
    &[
        key(Key::Shift, 9),
        ch('z', 'Z'),
        ch('x', 'X'),
        ch('c', 'C'),
        ch('v', 'V'),
        ch('b', 'B'),
        ch('n', 'N'),
        ch('m', 'M'),
        ch(',', '<'),
        ch('.', '>'),
        ch('/', '?'),
        key(Key::Shift, 11),
    ],
    &[
        key(Key::Ctrl, 6),
        key(Key::Super, 5),
        key(Key::Alt, 5),
        key(Key::Space, 28),
        key(Key::Alt, 5),
        key(Key::Super, 5),
        key(Key::Ctrl, 6),
    ],
];

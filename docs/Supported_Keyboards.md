# Supported Keyboards

The keyboards Keyloom recognises by their identifiers and draws as they
are printed. Every other keyboard is supported through the generic
detection described in
[Form_Factor_Detection.md](Form_Factor_Detection.md): its size is
guessed from the keys it reports and the digits in its name, and it is
drawn as one of the PC decks (100% down to 60%, ANSI or ISO) with the
standard legends. A keyboard listed here skips that guess.

The list is the table `KNOWN_KEYBOARDS` in [`src/known.rs`](../src/known.rs);
the two must say the same thing. When a model, a family, a deck, or the way
keyboards are identified changes, update this file and the "Recognised
keyboards" section of [Form_Factor_Detection.md](Form_Factor_Detection.md)
in the same change, and say which entries were verified on a keyboard.

## What "supported" means

For a recognised keyboard:

- The deck has the keyboard's arrangement and its printed legends: on an
  Apple keyboard, `⌘ command`, `⌥ option`, `⌃ control`, `⇧ shift`,
  `⌫ delete`, `⏎ return`, the media legends of the function row with the
  F number small, the Touch ID, lock, or eject key in the corner.
- Every key the keyboard sends is known: the tester names it, the key
  editor and the remaps list can map it, and the deck draws it.
- The keys are named as printed in the keyboard view, tester, key editor,
  and remaps list (Command rather than Super, Delete rather than
  Backspace). Stored mappings and the generated xremap configuration keep
  the standard names, so a mapping made on an Apple deck applies to the
  same physical key everywhere.
- The ANSI or ISO variant is read from the keyboard, not guessed.
- The settings of the kernel driver that translates the keyboard's keys
  are honoured, so each cap carries the code the key sends now.

It does not mean Touch ID authentication, which Linux does not support,
or any change to what the kernel does with the keys.

## Apple

Identified by Apple's vendor id (`05ac` over USB, `004c` over Bluetooth)
and the product id, which the kernel's `hid_apple` driver lists as well.
The same product id is used over both connections.

| Keyboard | Model | Product id | Deck | Function row | Corner key | Verified |
| --- | --- | --- | --- | --- | --- | --- |
| Magic Keyboard (2015) | A1644 | `0267` | Apple · Compact | Launchpad on F4, blank F5 and F6 | eject | from Apple's images and the kernel's tables |
| Magic Keyboard with Numeric Keypad (2017; sold with a USB-C cable since 2024) | A1843 | `026c` | Apple · Full-size, fn in the navigation cluster | Launchpad on F4, blank F5 and F6 | eject | from Apple's images and the kernel's tables |
| Magic Keyboard (2021) | A2450 | `029c` | Apple · Compact | Spotlight, Dictation, Do Not Disturb on F4 to F6 | lock | from Apple's images and the kernel's tables |
| Magic Keyboard with Touch ID (2021) | A2449 | `029a` | Apple · Compact | Spotlight, Dictation, Do Not Disturb | Touch ID | on the keyboard, over Bluetooth |
| Magic Keyboard with Touch ID and Numeric Keypad (2021) | A2520 | `029f` | Apple · Full-size, fn in the navigation cluster | Spotlight, Dictation, Do Not Disturb | Touch ID | from Apple's images and the kernel's tables |
| Magic Keyboard (USB-C, 2024) | A3203 | `0320` | Apple · Compact | Spotlight, Dictation, Do Not Disturb | lock | from Apple's images and the kernel's tables |
| Magic Keyboard with Touch ID (USB-C, 2024) | A3118 | `0321` | Apple · Compact | Spotlight, Dictation, Do Not Disturb | Touch ID | from Apple's images and the kernel's tables |
| Magic Keyboard with Touch ID and Numeric Keypad (USB-C, 2024) | A3119 | `0322` | Apple · Full-size, globe key at the bottom left and a contextual-menu key in the navigation cluster | Spotlight, Dictation, Do Not Disturb | Touch ID | from Apple's images and the kernel's tables |

The August 2026 revision of the USB-C keyboards changed only the
printing (symbols instead of words on tab, caps lock, shift, return,
delete, and on the keypad model's navigation and keypad keys); the decks
show the symbol with the word small, so both printings are covered.

An Apple product id not in the table is still drawn as an Apple keyboard:
the compact deck with the current legends, or the full-size deck when the
name says "Numeric Keypad". Keyboards that borrow Apple's vendor id
(Keychron and the other makers the kernel lists by name) are not treated
as Apple keyboards and get the generic detection.

### What is read from the keyboard and the driver

- **Variant.** ANSI, ISO, or JIS comes from the HID country code the
  kernel exposes beside the event node
  (`/sys/class/input/eventN/device/device/country`, in hex: `21` US,
  `0d` ISO, `0f` Japan; the European codes count as ISO). The kernel uses
  the same code to decide the ISO key swap below. JIS keyboards are
  recognised but drawn as ANSI for now.
- **The `hid_apple` driver's settings**, from
  `/sys/module/hid_apple/parameters`, read when the keyboard is seen:
  - `fnmode` decides whether the function row sends media keys with fn
    turning them into F1 to F12 (the default), or the other way round;
    the deck labels each key with the code it sends and shows the other
    legend small. The other set stays reachable through fn and "Remap a
    key that isn't shown".
  - `swap_opt_cmd`, `swap_ctrl_cmd`, and `swap_fn_leftctrl` move codes
    between the printed caps; the deck follows, so the cap printed
    `command` carries whatever code it sends.
  - `iso_layout` decides whether the driver swaps the codes of the key in
    the top-left corner and the key beside the left shift, which Apple's
    ISO keyboards report crosswise; the deck places the codes as sent.
  - A Magic Keyboard bound to `hid-generic` instead of `hid_apple` sends
    plain F keys and no fn key, and is drawn that way.

### Keys these keyboards send

Beyond the keys of a PC keyboard: brightness (`KEY_BRIGHTNESSDOWN`,
`KEY_BRIGHTNESSUP`), Mission Control (`KEY_SCALE`), Launchpad
(`KEY_DASHBOARD`), Spotlight (`KEY_SEARCH`), Dictation (`KEY_MICMUTE`),
Do Not Disturb (`KEY_SLEEP`), the media and volume keys, eject
(`KEY_EJECTCD`), the Touch ID or lock key (`KEY_POWER`, still to be
confirmed on a keyboard), fn (`KEY_FN`), F13 to F19, and the keypad's `=`
(`KEY_KPEQUAL`). All of them can be mapped.

Two of them reach the system as more than keys: logind watches the
keyboard for its power key, and `KEY_SLEEP` is the suspend key. Whether
pressing Touch ID or Do Not Disturb turns the computer off or suspends it
depends on the desktop's handling of those buttons; mapping the key to
Disabled or to something else in Keyloom stops it reaching the system as
that key.

### Limitations

- The driver's parameters are read when a keyboard is seen; a change to
  them takes effect after the keyboard reconnects or Keyloom restarts.
- Remapping fn changes what fn sends, not what it does to the function
  row, which the driver translates before any program sees the keys.
- A keyboard used over USB and over Bluetooth is two keyboards, as for
  every keyboard (they report different bus and vendor ids).
- The action catalog names the modifiers Super and Alt: a Command key
  is mapped by choosing Left Super.
- MacBook built-in keyboards and the aluminium keyboards from before
  2015 are not recognised yet; see
  [Upcoming_Features.md](Upcoming_Features.md).

## Adding a keyboard

1. Add a row to `KNOWN_KEYBOARDS` in `src/known.rs`: the vendor ids, the
   product id (the kernel's `drivers/hid/hid-ids.h` lists Apple's), the
   marketing name with the model number, the deck (an index into
   `keyboard::FORM_FACTORS`), and the details its deck needs. A new shape
   needs a deck builder in `src/ui/model.rs` and a form factor in
   `src/keyboard.rs`; a new family needs a variant of `known::Details`.
2. Add the same row to the table above, saying how it was verified.
3. Update the "Recognised keyboards" section of `Form_Factor_Detection.md`
   if the way keyboards are identified changed, and `Current_Features.md`
   if users gain a capability.
4. Check the decks with `cargo test` (geometry, identities, no overlap)
   and picture them with the screenshot tests
   (`cargo test --locked screenshots::decks -- --ignored`, which writes
   PNGs under `target/setup-shots/decks/`). With the keyboard attached,
   `cargo test --locked list_connected_keyboards -- --ignored --nocapture`
   prints what Keyloom makes of every keyboard on the machine: the model
   it recognised, the variant it read, and the driver's settings.

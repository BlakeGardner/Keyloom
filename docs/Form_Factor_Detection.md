# Form Factor and Layout Detection

How Keyloom decides which keyboard to draw: a physical size (form factor)
and an ANSI or ISO assembly. The deck follows the keyboard selected in
**Applies to** or Tester's **Listen to**. Detection provides defaults;
manual choices in **Size** are saved for that keyboard across selections,
reconnects, and app restarts, subject to the identity limitations below.

Keyboards Keyloom recognises by their identifiers (today Apple's Magic
Keyboards, listed in [Supported_Keyboards.md](Supported_Keyboards.md))
take both axes from what is known about the model instead of the guesses
described here; see "Recognised keyboards" below.

The implementation lives in [`src/keyboard.rs`](../src/keyboard.rs)
(the form factors and guessing), [`src/known.rs`](../src/known.rs)
(recognised models, the HID country code, the driver's settings),
[`src/monitor.rs`](../src/monitor.rs) (device identity, scanning, and
aggregation), [`src/ui/model.rs`](../src/ui/model.rs) (the decks), and
[`src/app.rs`](../src/app.rs) (selection and overrides).

## Selecting and overriding a keyboard

- Selecting one keyboard shows its saved choices, or its own detected size
  and ANSI/ISO variant by default. Other connected keyboards do not affect
  that deck.
- **Size** lists concrete sizes and ANSI/ISO variants, with the displayed
  values already selected and the detected values annotated. Selecting a
  value saves it for that scope independently of the other axis, even when
  it matches detection. There is no separate **Automatic** option.
- **All keyboards** has its own overrides. Its automatic size uses the
  aggregate described below; individual keyboard overrides do not change it.
- A disconnected selection keeps its last known guess and manual choices.
  Reconnecting a matching device restores the selection even if its event
  node changes. If an unrelated keyboard reuses the selected event node,
  selection returns to **All keyboards**. A keyboard not seen since launch,
  listed for its remaps, shows its manual choices, or its model's size and
  variant when Keyloom recognises it.
- Layout choices belong to keyboards, independently of remap profiles.
  Changing size or variant preserves mappings, including keys hidden by a
  smaller deck, and does not regenerate remaps or restart xremap.

## Supported sizes

| Form factor | What it renders |
| --- | --- |
| 100% · Full-size | F-row, core block, nav cluster, arrows, numpad |
| 80% · Tenkeyless | 100% without the numpad |
| 75% · Compact | Tight F-row, core block, one right-hand column, squeezed arrows |
| 65% | 75% without the F-row |
| 60% | Core block only |
| Apple · Compact | The 78-key Magic Keyboard: a full-height function row with a wide esc and the Touch ID, lock, or eject key in the corner; the 14.5-unit block with `⌫ delete`, `⏎ return`, and `⇧ shift`; fn, `⌃ control`, `⌥ option`, and `⌘ command` around the space bar; full-height ← and → with ↑ and ↓ stacked between them |
| Apple · Full-size | The Magic Keyboard with Numeric Keypad: the same block, F13 to F15 over the navigation cluster (fn or the contextual-menu key, home, page up, `⌦ delete`, end, page down) with full-height arrows, and F16 to F19 over the keypad (clear, =, /, *, a tall enter, a wide 0) |

Every size is available in ANSI and ISO. The ISO assembly narrows the
left Shift for the 102nd key, uses the two-segment tall Enter with the
`#`/`\` key beside it, and labels the right Alt as AltGr. The Apple ISO
assembly narrows the left shift the same way, puts the `§` key in the
corner and the `` ` `` key beside the shift, and keeps `\` beside the
lower segment of the tall return.

The Apple decks print their keys as Apple does: the symbol with the word
small (`⌘` over `command`), and on the function row the media legend of
each key with its F number small. A deck's cap carries the code the key
sends, so what a cap shows depends on the kernel driver's settings (see
"Recognised keyboards"). Any keyboard can be shown as an Apple deck from
the **Size** picker; without a recognised keyboard behind it, the deck is
the current Magic Keyboard with the driver's defaults.

## Recognised keyboards

A keyboard whose USB or Bluetooth vendor and product ids are in
`KNOWN_KEYBOARDS` (`src/known.rs`) skips the size and variant guesses:

1. **Model.** The product id names the model, which fixes the deck (an
   index into the form factors), the generation of legends on its function
   row, and the key in its corner. Apple uses one product id per model over
   both USB (vendor `05ac`) and Bluetooth (vendor `004c`). Keyboards that
   borrow Apple's vendor id (Keychron and the other makers the kernel's
   `hid_apple` driver lists by name) are excluded by name and guessed like
   any other keyboard. An Apple product id not in the table is drawn as the
   current compact Magic Keyboard, or as the full-size one when the device
   name says "Numeric Keypad".
2. **Variant.** Unless the product id fixes it, the physical variant comes
   from the HID country code the kernel exposes beside the event node,
   `/sys/class/input/eventN/device/device/country` (hexadecimal): `0d`,
   the ISO keyboard, and the European countries mean ISO; `0f`, Japan,
   means JIS; the US (`21`), Latin America, and an unspecified code (`00`)
   mean ANSI. The `KEY_102ND` test below is not applied to a recognised
   keyboard: its driver advertises that key whatever the variant. A JIS
   keyboard is recognised as such but drawn as ANSI for now.
3. **The driver's settings.** Apple keyboards are driven by the kernel's
   `hid_apple` module, which translates keys before any program sees
   them, so the deck reads its parameters
   (`/sys/module/hid_apple/parameters`) when the keyboard is seen:
   `fnmode` decides whether the function row sends media keys or F keys by
   default (fn held sends the other set), and `swap_opt_cmd`,
   `swap_ctrl_cmd`, `swap_fn_leftctrl`, and `iso_layout` move codes between
   caps. Each cap carries the code its key sends now, with the other legend
   small; the set behind fn stays reachable through "Remap a key that isn't
   shown". Which driver has the keyboard is read from sysfs as well: under
   `hid-generic` the function row is plain F keys and no fn key is
   reported, and the deck says so. A parameter changed while Keyloom runs
   is noticed when the keyboard reconnects.

A recognised keyboard counts as name-hinted in the aggregation below, so
its deck is never displaced by another keyboard's capability guess. Its
identity for saved choices is the same as any keyboard's.

## Size detection

Each readable keyboard in `/dev/input` that is not recognised gets its own
guess from two signals, combined per device. Aggregation is used only for
**All keyboards**.

### 1. Capability scan

evdev devices report the set of keys they can emit. The guess walks down
a cascade (`form_for_keys`):

1. reports a numpad key (`KP0`) → **100%**
2. reports the nav cluster (`Scroll Lock` *and* `Pause`) → **80% TKL**
3. reports an F-row (`F1`) → **75%**
4. reports arrow keys → **65%**
5. otherwise → **60%**

The direction of trust matters: **absence is meaningful, presence is
not**. A device that does not report numpad scancodes conclusively has no
numpad, but compact boards routinely report every key their Fn layer can
emit — a 65% board often claims F-keys and a numpad it doesn't physically
have. Laptop internal keyboards do the same, and so does a compact Magic
Keyboard, which is why Apple's keyboards are recognised rather than
guessed. This is why the capability scan alone tends to over-estimate the
size.

### 2. Name hint

Keyboards routinely encode their size or key count in the product name,
which is often more truthful than the reported capabilities
(`form_for_name`). The name is split into runs of digits, and the first
recognized run wins:

| Digit run | Size |
| --- | --- |
| 96, 98, 100, 104, 108 | 100% |
| 80, 87, 88 | 80% TKL |
| 75, 84 | 75% |
| 65, 66, 67, 68 | 65% |
| 60, 61 | 60% |

Examples: `@HFD NEO80 Keyboard` → TKL, `Redragon K552-87` → TKL,
`Epomaker TH84` → 75%, `Keychron Q65` → 65%, `SKYLOONG GK61` → 60%.
Because only whole digit runs are matched, unrelated numbers
(`TESmart DKS202-P24`, `Logitech MX Keys`) produce no hint.

### Combining the two

A name hint may only **shrink** the capability guess (`max` of the two,
since among the PC sizes smaller sizes have higher indices). Rationale:
the capability scan can only err toward bigger boards, so a smaller name
hint corrects it — but a bigger name hint never overrules conclusively
missing keys.

### Aggregating multiple keyboards

With **All keyboards** selected (`monitor::detected_form`):

1. Physical devices are preferred over software-created virtual devices, so
   broad uinput capabilities from tools such as Solaar or xremap do not
   distort the physical keyboard layout. Virtual devices remain a fallback
   when no physical keyboard is visible.
2. Devices whose guess used a name hint, and recognised keyboards, are
   preferred over capability-only guesses, since those are more
   trustworthy.
3. Within the preferred group, the **largest** board wins, compared by a
   size rank that spans both families (`keyboard::larger`): a full-size
   Apple keyboard and a 100% PC keyboard rank the same, and the one listed
   first wins a tie.

Disconnected devices are ignored. The result updates on startup, connection,
disconnection, and switching back to **All keyboards**, unless that scope
has a manual size override. With no detected devices, automatic size falls
back to 100%. When the aggregate lands on an Apple deck, it is drawn for
the first connected physical keyboard of that form.

## ANSI/ISO detection

For a keyboard that is not recognised, ANSI/ISO detection uses the keys
each evdev device reports. A keyboard that reports `KEY_102ND`, the extra
physical key beside the left Shift on an ISO board, is detected as ISO;
otherwise it is detected as ANSI. This uses the same Linux input interface
as size detection and does not read settings from COSMIC, another desktop
environment, a compositor, or a distribution file. A recognised keyboard
uses its HID country code instead (see above).

With one keyboard selected, its own result is used. **All keyboards** uses ISO
if any connected physical keyboard reports the extra key, so the combined
deck does not hide a physical key. Virtual devices are considered only when
no physical keyboard is visible. A manual ANSI or ISO choice overrides the
guess for that scope.

## Saved identity

Overrides are stored in the `keyboard_layouts` entry of Keyloom's
cosmic-config settings. They use bus, vendor, and product identifiers plus
the first available discriminator:

1. the device's unique identifier (such as a serial number),
2. its physical connection path,
3. its name.

The `/dev/input/eventN` path is not part of this saved identity. Existing
settings without keyboard overrides continue using automatic defaults.

## Known limitations

- Without read access to `/dev/input` (the `input` group), no devices are
  visible; automatic size defaults to 100% and automatic variant defaults to
  ANSI. **All keyboards** overrides remain available.
- Over-reporting laptop and compact keyboards without a size in their
  name are typically detected one size too large. That is the expected
  trade-off — pick the right size manually and the choice is saved.
- The name-hint table only understands the common size and key-count
  conventions listed above.
- Some keyboard firmware may report `KEY_102ND` even when the key is available
  only through a layer, or omit it despite having an ISO-shaped case. The
  ANSI/ISO override handles those devices.
- Without a unique identifier, a keyboard moved to a different port may be
  treated as a new device. Identical models exposing neither a unique
  identifier nor a physical path share overrides when their names match.
  Different connection modes (such as USB and Bluetooth) may also be treated
  as separate devices.
- A recognised keyboard's JIS variant is drawn as ANSI, and the driver's
  parameters are read when the keyboard is seen, so a change to them shows
  after a reconnect. The keys of an Apple keyboard that reach the system as
  buttons (the Touch ID or lock key as the power key, Do Not Disturb as the
  suspend key) are listed in [Supported_Keyboards.md](Supported_Keyboards.md).

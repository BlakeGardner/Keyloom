# Form Factor and Layout Detection

How Keyloom decides which keyboard to draw: a physical size (form factor)
and an ANSI or ISO assembly. The deck follows the keyboard selected in
**Applies to** or Tester's **Listen to**. Detection provides defaults;
manual choices in **Size** are saved for that keyboard across selections,
reconnects, and app restarts, subject to the identity limitations below.

The implementation lives in [`src/keyboard.rs`](../src/keyboard.rs)
(guessing), [`src/monitor.rs`](../src/monitor.rs) (device identity, scanning,
and aggregation), and [`src/app.rs`](../src/app.rs) (selection and overrides).

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
  selection returns to **All keyboards**.
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

Every size is available in ANSI and ISO. The ISO assembly narrows the
left Shift for the 102nd key, uses the two-segment tall Enter with the
`#`/`\` key beside it, and labels the right Alt as AltGr.

## Size detection

Each readable keyboard in `/dev/input` gets its own guess from two
signals, combined per device. Aggregation is used only for **All keyboards**.

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
have. Laptop internal keyboards do the same. This is why the capability
scan alone tends to over-estimate the size.

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
since smaller sizes have higher indices). Rationale: the capability scan
can only err toward bigger boards, so a smaller name hint corrects it —
but a bigger name hint never overrules conclusively missing keys.

### Aggregating multiple keyboards

With **All keyboards** selected (`monitor::detected_form`):

1. Physical devices are preferred over software-created virtual devices, so
   broad uinput capabilities from tools such as Solaar or xremap do not
   distort the physical keyboard layout. Virtual devices remain a fallback
   when no physical keyboard is visible.
2. Devices whose guess used a name hint are preferred over
   capability-only guesses, since hints are more trustworthy.
3. Within the preferred group, the **largest** board wins.

Disconnected devices are ignored. The result updates on startup, connection,
disconnection, and switching back to **All keyboards**, unless that scope
has a manual size override. With no detected devices, automatic size falls
back to 100%.

## ANSI/ISO detection

ANSI/ISO detection uses the keys each evdev device reports. A keyboard that
reports `KEY_102ND`, the extra physical key beside the left Shift on an ISO
board, is detected as ISO; otherwise it is detected as ANSI. This uses the
same Linux input interface as size detection and does not read settings from
COSMIC, another desktop environment, a compositor, or a distribution file.

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

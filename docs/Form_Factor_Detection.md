# Form Factor and Layout Detection

How Keyloom decides which keyboard to draw when it starts: a physical
size (form factor) and an ANSI or ISO assembly. Detection only picks
defaults — the **Size** picker in the keyboard toolbar always wins, and a
manual choice is never overridden for the rest of the session.

The implementation lives in [`src/keyboard.rs`](../src/keyboard.rs)
(guessing) and [`src/monitor.rs`](../src/monitor.rs) (per-device scan and
aggregation).

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
signals, combined per device and then aggregated.

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

With several keyboards connected (`detected_form`):

1. Devices whose guess used a name hint are preferred over
   capability-only guesses, since hints are more trustworthy.
2. Within the preferred group, the **largest** board wins, so no
   physically present key is hidden from the deck.

Disconnected devices are ignored. The result is applied when the startup
device scan reports in and again whenever a keyboard is plugged in later —
but only until the user picks a size manually.

## ANSI/ISO detection

The variant comes from the system's configured XKB layout (`detect_iso`),
checked in this order:

1. the COSMIC compositor configuration
   (`~/.config/cosmic/com.system76.CosmicComp/v1/xkb_config`),
2. the `XKB_DEFAULT_LAYOUT` environment variable (honored by wlroots
   compositors and others),
3. `/etc/default/keyboard` (`XKBLAYOUT=`).

The first configured layout in a comma-separated list is matched against
a table of ISO-assembly layouts (`gb`, `de`, `fr`, `es`, `it`, the
Nordics, and most other European layouts). `us` and anything unrecognized
default to ANSI.

## Known limitations

- Without read access to `/dev/input` (the `input` group), no devices are
  visible and the deck defaults to 100%.
- Over-reporting laptop and compact keyboards without a size in their
  name are typically detected one size too large. That is the expected
  trade-off — pick the right size manually and the choice sticks for the
  session.
- The name-hint table only understands the common size and key-count
  conventions listed above.

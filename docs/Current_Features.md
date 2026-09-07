# Current Features

What Keyloom does today. This is the counterpart to
[Functionality_TODO.md](Functionality_TODO.md), which tracks what is still
missing; [First_Release_Scope.md](First_Release_Scope.md) defines the v0.1
target.

**The big caveat:** shortcut groups are an in-memory preview and live only
for the current session. Profiles and key mappings, however, are persisted
via cosmic-config and translated into an xremap configuration file at
`~/.config/xremap/config.yml` on every change (see "Persistence and
generated configuration" below). Keyloom does not run or manage the xremap
service itself yet.

## Application shell

- Native Rust + libcosmic application; runs on Wayland and X11.
- Follows the system light/dark theme.
- Header with brand, profile switcher, view tabs (Keyboard / Tester /
  Shortcuts), and an overflow menu (show first-run setup, reset all mappings,
  about).
- Three-step onboarding flow on first launch, including a one-click
  Caps Lock → Escape example; can be skipped or reopened from the menu.
- Confirmation toasts for every destructive or notable change, with a working
  Undo action and timed dismissal.
- Escape closes the topmost surface first (popover → dialog → onboarding →
  editor sheet), like a native app.
- Selection editors slide in as an animated bottom sheet; dialogs render as
  modal overlays.

## Keyboard view

- Full 100% ANSI deck rendered key-by-key with design-accurate geometry
  (function row, nav cluster, numpad).
- Keys light up live as they are pressed on any monitored physical keyboard.
- Clicking a key opens the key editor; mapped keys show their new action on
  the cap (tap and hold legends), and swap/disabled states are styled.
- Layer preview: a toggleable Navigation layer shows what H/J/K/L and friends
  become while Caps Lock is held, with the trigger key highlighted.
- Mapping summary under the deck: chips for every mapping in the active
  profile, an empty-state prompt when there are none, and a
  "View remaps · N" button.
- Remaps dialog listing every mapping in the active profile; selecting an
  entry opens it in the editor.
- "Applies to" device scope selector on the toolbar (see Devices below).

## Key editor (bottom sheet)

- Searchable action catalog with categories: modifiers, navigation, media,
  letters, numbers, function keys, punctuation, other (including Disabled).
- The category is preselected to match the clicked key.
- "Record a key" capture mode: press a physical key to use it as the output,
  with Escape to cancel.
- Advanced options per key:
  - **Normal press** (tap) and **When held** (hold) actions.
  - **With other keys** (combo): build modifier-chord rules from Ctrl / Shift /
    Alt / Super chips on both input and output sides; these appear in the
    Shortcuts view under a "From the keyboard" group.
  - **Two-way swap** toggle between the key and its tap action.
- "Restore original key" clears the mapping.
- Existing combos for the key are listed and removable inline.

## Profiles

- Multiple named profiles, each with its own mappings and shortcut groups.
- Profile switcher popover with new-profile and duplicate-profile actions.
- Ships with demo profiles (Default, Laptop, Mac-style, Gaming, Mac + Cosmic)
  that showcase mappings, swaps, device scopes, and shortcut groups.
- "Reset all mappings" clears the active profile (undoable).

## Devices

- Global keyboard monitoring via evdev (`/dev/input/event*`), non-exclusive —
  the compositor still receives every event.
- Detects readable keyboards at startup, distinguishes real keyboards from
  other input devices, and marks keyboards that disconnect.
- Per-mapping device scope: "All keyboards" or one specific keyboard.
  Detected keyboards replace the demo device list as soon as they appear.
- Best-effort form-factor guess per device from its name and reported keys
  (not yet used by the UI).

## Tester view

- Shows the last observed key: big key cap, friendly name, key code, and the
  keyboard it came from.
- Live held-modifier chips (Shift / Control / Alt / Super).
- "Becomes" panel showing what the key turns into under the active profile,
  or "no mapping — passes through".
- Works before any mapping exists and never modifies state — editing messages
  are ignored while the tester is open.

## Shortcuts view

- Shortcut groups per profile, each with a name, enable/disable toggle, an
  application scope label (all applications or named apps), and an
  "any modifier" matching option.
- Rules shown as chord pills: modifiers + key → modifiers + key, with notes.
- Rule editor with **Record** buttons for both sides: hold modifiers and press
  a key on the physical keyboard to capture a chord; Escape cancels.
- Add group, delete rule, and edit existing rules.
- Demo groups on the Mac + Cosmic profile (desktop shortcuts, terminal
  copy/paste translation, media keys that ignore modifiers).

## Persistence and generated configuration

- Profiles and their mappings (plus the active profile) are stored via
  cosmic-config under `~/.config/cosmic/io.github.blakegardner.Keyloom/`
  and restored on launch; a fresh install seeds the demo profiles.
- Every mapping or profile change regenerates a deterministic xremap
  document from the internal rule model and writes it to
  `$XDG_CONFIG_HOME/xremap/config.yml` (usually `~/.config`), so identical
  mappings always produce byte-identical YAML.
- The generator translates friendly action names into xremap `KEY_*` names,
  emits one `modmap` block for unscoped mappings plus one per device scope
  (`device.only`), renders tap/hold as `held`/`alone`, two-way swaps as two
  entries, and Disabled keys as an empty output (`[]`).
- Files Keyloom generated carry a marker comment; a hand-written xremap
  config found at that path is backed up to `config.yml.bak` before the
  first overwrite, and is never touched just for launching the app.
- Shortcut groups are not yet part of the generated output or the stored
  model.

## Layout engine (currently dormant)

`src/keyboard.rs` contains a full layout system from before the redesign:
form factors (100%, TKL, 75%, 65%, 60%), ANSI/ISO variants, and per-key
slot assembly. The new UI renders only the 100% ANSI deck and does not expose
it yet — reconnecting it is tracked in
[Functionality_TODO.md](Functionality_TODO.md).

## Test coverage

- Unit tests for the update logic in `src/app.rs`: mapping assignment, undo,
  toast lifecycle, profile switching and duplication, editor/sheet state
  transitions, hold mappings, combo rules, and onboarding.
- Generator tests in `src/xremap.rs`: key-name translation coverage,
  deterministic golden output, tap/hold, swaps, disabled keys, device
  scoping, and foreign-file backup behavior. Generated documents
  (including every source key and every catalog action) are additionally
  parsed by a real xremap binary when one is on PATH — CI installs the
  pinned release (0.15.12) so this always runs there.
- Store round-trip tests in `src/config.rs`.
- Layout assembly tests in `src/keyboard.rs`.

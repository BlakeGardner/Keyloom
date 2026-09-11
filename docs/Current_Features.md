# Current Features

What Keyloom does today. This is the counterpart to
[Functionality_TODO.md](Functionality_TODO.md), which tracks what is still
missing; [First_Release_Scope.md](First_Release_Scope.md) defines the v0.1
target.

**The big caveat:** shortcut groups are an in-memory preview and live only
for the current session. Profiles and key mappings, however, are persisted
via cosmic-config and translated into an xremap configuration file at
`~/.config/xremap/config.yml` on every change (see "Persistence and
generated configuration" below). Keyloom applies changes by restarting an
existing `xremap.service` systemd user unit automatically, but it does not
yet install xremap, register the unit, or set up permissions.

## Application shell

- Native Rust + libcosmic application; runs on Wayland and X11.
- Follows the system light/dark theme.
- Header with brand, profile switcher, view tabs (Keyboard / Tester /
  Shortcuts), a remap status chip (Remapping Enabled / Off / Failed /
  not set up / Unavailable, plus a transient Applying Remaps state while a
  change is applied — generic wording that never names xremap; click to
  re-check), and an overflow menu (show first-run setup, reset all
  mappings, about).
- Three-step onboarding flow available from "Show first-run setup" in the
  menu, including a one-click Caps Lock → Escape example; can be skipped
  or reopened. It does not yet open automatically on first launch.
- About Keyloom modal using the same centered card and dimmed backdrop as
  onboarding, with an embedded SVG logo, build version from Cargo, a short
  application description, Rust / libcosmic / xremap credits, and GPLv3-only
  license information with a no-warranty notice. Available
  from every view; dismiss with Close, Escape, or a backdrop click. The logo
  lives in `assets/keyloom_logo.svg` and is compiled into the application.
- Confirmation toasts for every destructive or notable change, with a working
  Undo action and timed dismissal.
- While Keyloom has focus, Escape dismisses only the topmost dialog, popover,
  recording mode, or editor sheet. Global keyboard monitoring does not dismiss
  these surfaces when Escape is pressed in another application.
- Selection editors slide in as an animated bottom sheet; dialogs render as
  modal overlays.
- Clicking text, padding, or unused space inside a custom dialog keeps it
  open. Dialog controls remain interactive, and clicking outside the card
  dismisses it. This applies to remaps, key capture, deletion confirmation,
  About, and onboarding.

## Keyboard view

- Renders every form factor (100%, 80% TKL, 75%, 65%, 60%) in ANSI and ISO
  assemblies. The 100% and TKL decks use the design export's exact
  geometry; compact sizes follow the classic assemblies (right-hand
  column, ↑ carved from right Shift, squeezed bottom row), and ISO adds
  the 102nd key, two-segment tall Enter, and AltGr.
- The deck defaults to the detected form factor and ANSI/ISO variant (see
  [Form_Factor_Detection.md](Form_Factor_Detection.md)); the toolbar's
  Size picker overrides both for the session, and mappings survive deck
  switches untouched.
- Keys light up live as they are pressed on any monitored physical keyboard.
- Clicking a key opens the key editor; mapped keys show their new action on
  the cap (tap and hold legends), and swap/disabled states are styled.
- Layer preview: a toggleable Navigation layer shows what H/J/K/L and friends
  become while Caps Lock is held, with the trigger key highlighted.
- Mapping summary under the deck: chips for every mapping in the active
  profile, an empty-state prompt when there are none, and a
  "View remaps · N" button.
- Remaps dialog listing every mapping in the active profile; selecting an
  entry opens it in the editor, and each row has the same small red X button
  as the profile switcher, using the same native button pointer cursor as
  the adjacent remap row, without a tooltip overlay. The scrollbar has its
  own space beside the row controls. Removal requires confirmation; Cancel,
  Escape, or a backdrop click returns to the list
  without changes. Confirmed removal is undoable, and the list stays open
  with an empty state once the last mapping is gone.
- "Applies to" device scope selector on the toolbar (see Devices below).

## Key editor (bottom sheet)

- Searchable action catalog with categories: modifiers, navigation, media,
  letters, numbers, numpad, function keys, punctuation, other (including
  Disabled). Modifiers name their side explicitly (Left/Right Control, Shift,
  Alt, Super), and the catalog offers every key the app renders.
- The category is preselected to match the clicked key.
- Mapping a key to itself is rejected with a short explanation, except when a
  different hold action makes the self-tap meaningful.
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
- Profile switcher popover with rename, new-profile, and duplicate-profile
  actions, plus per-row deletion behind a confirmation dialog. The active
  profile can't be deleted, so at least one profile always remains;
  deletion is undoable from the toast. While closed, the switcher shows
  a tooltip explaining what profiles are.
- A fresh install seeds an empty Default profile plus editable starter
  profiles (Laptop, Mac-style, Gaming, Media F-row). They are ordinary
  profiles from then on: renamed, edited, and stored like any other.
- "Reset all mappings" clears the active profile (undoable).

## Devices

- Global keyboard monitoring via evdev (`/dev/input/event*`), non-exclusive —
  the compositor still receives every event.
- Detects readable keyboards at startup, distinguishes real keyboards from
  other input devices, and marks keyboards that disconnect.
- Keyboards plugged in after launch are picked up by a background scan of
  `/dev/input` and announced with a toast; a replugged keyboard replaces its
  stale entry, and a selected device scope follows it to the new node.
- Per-mapping device scope: "All keyboards" or one specific detected
  keyboard, with an explicit empty state when none are readable.
- Best-effort form-factor guess per device from its name and reported keys,
  used to pick the default deck (see
  [Form_Factor_Detection.md](Form_Factor_Detection.md)).

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
- Groups start empty for every profile; combo mappings made in the key
  editor appear under an automatic "From the keyboard" group.

## Persistence and generated configuration

- Profiles and their mappings (plus the active profile) are stored via
  cosmic-config under `~/.config/cosmic/io.github.blakegardner.Keyloom/`
  and restored on launch; a fresh install seeds the Default profile and
  the starter profiles, which persist like any other from then on.
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

## Applying (xremap service)

- Changes apply themselves — there is no Apply button. Every change that
  actually rewrites the generated config schedules a restart of the
  `xremap.service` systemd user unit (`systemctl --user restart`) after a
  short debounce, so xremap re-reads the file; restarts are paced to stay
  under systemd's default start rate limit. Success is silent (the change's
  own toast is the confirmation); failures surface systemctl's error as a
  toast.
- The unit's state is queried at startup (`systemctl --user show`) and shown
  as a chip in the header with generic wording: Remapping Enabled, Remapping
  Off, Remapping Failed, Remapping not set up (no such unit), or Remapping
  Unavailable (no systemd). While a change is on its way to the service —
  from the debounce until the restart settles — the chip shows Applying
  Remaps with an amber dot. Clicking the chip re-checks. An absent or broken
  service never blocks editing, and auto-apply skips restarting when there
  is no unit to restart.
- Keyloom assumes xremap is installed and the user unit plus permissions are
  already set up; installing or registering them is not handled yet (see
  [Functionality_TODO.md](Functionality_TODO.md) §6 and §9).

## Layout engine (retired)

The pre-redesign layout system was retired: form factors and ANSI/ISO
assemblies are now first-class in the new UI (`src/ui/model.rs` builds the
decks), and `src/keyboard.rs` retains only size/variant detection. The old
language charmaps and typed-text preview were removed with it.

## Test coverage

- Unit tests for the update logic in `src/app.rs`: mapping assignment, undo,
  toast lifecycle, profile switching, duplication, renaming, deletion,
  starter-profile seeding and editing, editor/sheet state transitions,
  hold mappings, combo rules, onboarding, apply/service-status handling,
  and device hotplug/replug handling.
- Generator tests in `src/xremap.rs`: key-name translation coverage,
  deterministic golden output, tap/hold, swaps, disabled keys, device
  scoping, and foreign-file backup behavior. Generated documents
  (including every source key and every catalog action) are additionally
  parsed by a real xremap binary in CI, which installs the pinned release
  (0.15.12) and explicitly runs the ignored integration test. A missing
  binary fails that CI check.
  Run the local suite with `cargo test`; it does not launch xremap, even
  when xremap is installed on PATH. The real-binary check is reported as
  ignored, while the generator unit tests still run.
- Store round-trip tests in `src/config.rs`.
- Deck assembly tests in `src/ui/model.rs` (per-form clusters, ISO
  transform, overlap and identity checks) and detection tests in
  `src/keyboard.rs`.

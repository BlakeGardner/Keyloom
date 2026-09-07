# Functionality TODO

Tracks the functionality still needed to make Keyloom complete, based on the
current UI and the known scope in [First_Release_Scope.md](First_Release_Scope.md)
(v0.1, takes precedence) and [Product_Plan.md](Product_Plan.md) (long-term vision).

The UI shell is largely in place — see [Current_Features.md](Current_Features.md)
for what exists today. Everything below is behavior that the UI previews but the
program does not actually do yet. Each task is meant to be small enough to
pick up on its own.

## 1. YAML generation and copy (v0.1 release blockers)

> **Direction change:** instead of a copy-paste YAML panel, the generated
> configuration is now written automatically to
> `$XDG_CONFIG_HOME/xremap/config.yml` on every change (see `src/xremap.rs`).
> The preview/copy items below are superseded by that behavior.

- [x] **Action → xremap key-name table.** Translate the editor's friendly action
  names (e.g. `Escape`, `Right Control`) into recognized xremap key names
  (`KEY_ESC`, `KEY_RIGHTCTRL`). The physical-key side can derive from the
  existing evdev codes in `ui/model.rs`.
- [x] **Deterministic YAML generator.** Produce a complete document with one
  `modmap` block from the active mappings: no duplicate source keys, stable
  ordering, identical output for identical mappings.
- [ ] ~~**Read-only YAML preview panel.**~~ Superseded: the config is written to
  disk automatically; an advanced inspect option may return later.
- [ ] ~~**Copy YAML button.**~~ Superseded by automatic writing.
- [ ] ~~**Empty state for the preview.**~~ Superseded: an empty profile writes
  `modmap: []`.
- [ ] **Boundary messaging.** Make it clear in the UI that the config is
  written for xremap but the service is not yet managed by this app.
- [ ] ~~**Session-only notice.**~~ Superseded: mappings now persist via
  cosmic-config.
- [x] **Validate against a recorded xremap version.** Check representative
  generated configs (simple remap, modifier remap, two-rule swap) against a
  pinned xremap release during development. (Automated: the test suite
  parses generated documents with a real xremap binary, and CI installs the
  pinned release so it runs on every push.)

## 2. Mapping rule correctness (v0.1)

- [ ] **Reject mapping a key to itself** with a short explanation instead of
  silently accepting it.
- [ ] **Audit left/right modifier coverage in the action catalog.** `Right
  Shift` is missing, and left-hand entries should be explicit about their side
  so left/right stay distinct in generated rules.
- [ ] **Cover numpad keys in the output picker.** The catalog should offer every
  key the app renders, including keys outside the displayed form factor.
- [ ] **Remove a mapping directly from the mapping list.** The remaps dialog
  currently only opens a mapping for editing; add inline removal.

## 3. Layouts and form factors

The pre-redesign layout system in `keyboard.rs` (100% / TKL / 75% / 65% / 60%,
ANSI/ISO) is no longer reachable from the new UI, which renders only the 100%
ANSI deck.

- [ ] **Re-expose form-factor selection** in the new UI.
- [ ] **Re-expose layout variants** (ISO and other assemblies from
  `keyboard.rs`).
- [ ] **Preserve mappings across layout/form-factor changes.** Mappings key off
  physical identity, so switching the displayed deck must not change or delete
  rules; verify and add a regression test.
- [ ] **Default the deck to the detected form factor.** The monitor already
  guesses a form factor per device (`detected_form`); use it to pick the
  initial deck instead of always showing 100%.

## 4. Real data instead of demo data

- [ ] **Start new installs with an empty Default profile** instead of the demo
  profiles (`Laptop`, `Mac-style`, `Gaming`, `Mac + Cosmic`), or gate the demo
  content behind onboarding as an explicit example.
- [ ] **Drop the demo shortcut groups** the same way once real groups can be
  created and kept.
- [ ] **Remove the demo device entries** once detected keyboards are reliable;
  today they only disappear when the monitor finds real devices.
- [ ] **Detect keyboards plugged in after launch.** The monitor reports the
  startup device list and disconnections, but never adds a newly connected
  keyboard.

## 5. Persistence ("Save" iteration)

- [ ] **Save YAML to a user-selected file** via a native file dialog.
- [x] **Persist profiles and mappings between sessions** via cosmic-config
  (`src/config.rs`; the rule model stays the source of truth and YAML remains
  generated output). Shortcut groups are not persisted yet.
- [ ] **Remember UI state** worth keeping across launches (active profile is
  remembered; selected device scope and onboarding-completed flag are not).

## 6. Apply and service management ("Apply" / "Manage" iterations)

- [ ] **Detect an xremap installation** and show its status without blocking
  any editing or copying when it is absent.
- [x] **Write the generated config to the user's xremap config path** with
  validation and clear failure feedback. (Written to
  `$XDG_CONFIG_HOME/xremap/config.yml`; hand-written files are backed up
  before the first overwrite, and write failures surface as a toast.)
- [ ] **Reload/apply on demand** so saved changes take effect.
- [ ] **Service controls:** status display plus explicit start, stop, reload,
  and restart.
- [ ] **Permission guidance.** Detect missing `/dev/input` read access (the
  `input` group) and walk the user through fixing it instead of failing
  silently.

## 7. Advanced remapping in generated config ("Expand" iteration)

Everything here is already editable in the UI; the unchecked items are
still only previewed in memory.

- [x] **Tap/hold mappings** → xremap `held`/`alone` output.
- [x] **Two-way swaps** → two explicit generated entries.
- [x] **Disabled keys** → generated no-op mapping (empty output list).
- [x] **Device-scoped mappings** → per-device `modmap` sections keyed to the
  selected keyboard.
- [ ] **Shortcut groups (chord → chord rules)** → xremap `keymap` blocks,
  including the "any modifier" matching option.
- [ ] **Application-scoped groups** → xremap `application` filters, with a
  picker for real window classes instead of free-form names.
- [ ] **Layers** (the previewed Caps-Lock nav layer) → generated layer config.

## 8. App polish and distribution

- [ ] **Real About dialog** with version and license info (currently a toast).
- [ ] **Desktop entry and icon** in `data/` so the app installs and launches
  from a menu.
- [ ] **Choose a license** (README currently says TBD).

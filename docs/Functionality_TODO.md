# Functionality TODO

Tracks the functionality still needed to make Keyloom complete, based on the
current UI and the known scope in [First_Release_Scope.md](First_Release_Scope.md)
(v0.1, takes precedence) and [Product_Plan.md](Product_Plan.md) (long-term vision).

The UI shell is largely in place — see [Current_Features.md](Current_Features.md)
for what exists today. Unchecked items track missing functionality and
follow-up work; checked items record completed work. Each task is meant to
be small enough to pick up on its own.

## 1. Generated configuration (v0.1)

Configuration is generated and written automatically to
`$XDG_CONFIG_HOME/xremap/keyloom.yml` (see `src/xremap.rs`). A copy-only YAML
workflow is not planned.

- [x] **Action → xremap key-name table.** Translate the editor's friendly action
  names (e.g. `Escape`, `Right Control`) into recognized xremap key names
  (`KEY_ESC`, `KEY_RIGHTCTRL`). The physical-key side can derive from the
  existing evdev codes in `ui/model.rs`.
- [x] **Deterministic YAML generator.** Produce a complete document with one
  `modmap` block from the active mappings: no duplicate source keys, stable
  ordering, identical output for identical mappings.
- [x] **Validate against a recorded xremap version.** Check representative
  generated configs (simple remap, modifier remap, two-rule swap) against a
  pinned xremap release in CI. (Automated: CI installs the pinned release
  and explicitly runs the real-binary integration test. Normal local test
  runs ignore that test and do not launch the installed xremap.)

## 2. Mapping rule correctness (v0.1)

- [x] **Reject mapping a key to itself** with a short explanation instead of
  silently accepting it. (No-op self-maps are rejected with a toast; setting
  the tap back to the key itself stays allowed when a different hold action
  makes it meaningful.)
- [x] **Audit left/right modifier coverage in the action catalog.** `Right
  Shift` is missing, and left-hand entries should be explicit about their side
  so left/right stay distinct in generated rules. (The catalog now names every
  modifier side explicitly; older generic names remain accepted from stored
  mappings.)
- [x] **Cover numpad keys in the output picker.** The catalog should offer every
  key the app renders, including keys outside the displayed form factor.
  (Added a Numpad group plus Scroll Lock and Pause; a test asserts the catalog
  offers every rendered key under its display name.)
- [x] **Remove a mapping directly from the mapping list.** The remaps dialog
  supports inline removal through the same small red X button used in the
  profile switcher, with a confirmation dialog. The X uses native button
  hover handling without a tooltip overlay, matching the adjacent remap row.
  The scrollbar occupies a separate gutter so it cannot cover the buttons
  or block their pointer cursor on hover.
  Cancel, Escape, and backdrop clicks preserve the mapping and return to
  the list; confirmed removal is undoable, with an empty state after the
  last mapping is removed.

## 3. Layouts and form factors

- [x] **Re-expose form-factor selection** in the new UI. (The keyboard
  toolbar's Size picker offers 100% / TKL / 75% / 65% / 60%.)
- [x] **Re-expose layout variants.** (ANSI and ISO assemblies for every
  size; the ISO variant is auto-detected from the configured XKB layout.
  The pre-redesign language charmaps and typed-text preview were dropped
  with the old layout engine.)
- [x] **Preserve mappings across layout/form-factor changes.** Mappings key off
  physical identity, so switching the displayed deck must not change or delete
  rules; verified with a regression test (including unchanged generated YAML).
- [x] **Default the deck to the detected form factor.** The monitor's
  per-device guess (`detected_form`) picks the initial deck; a manual size
  choice always wins. See
  [Form_Factor_Detection.md](Form_Factor_Detection.md).

## 4. Real data instead of demo data

- [x] **Keep live key mirroring in Tester only.** The remap deck no longer
  lights up for physical key presses, avoiding input from unrelated keyboards
  appearing under a selected "Applies to" scope. Tester retains device-filtered
  highlights, and explicit key recording remains available in the editor.
  Regression coverage checks both scopes, switching views, and key capture.
- [x] **Filter tester input by the selected keyboard.** The "Listen to"
  selector filters physical last-key updates, key highlights, and modifier
  chips; "All keyboards" restores aggregate input. Changing the selection
  clears the last-key preview and immediately updates held keys. The selected
  device remains shared with the editor's mapping scope; manual key-cap
  previews remain available.
- [x] **Start new installs with an empty Default profile** instead of the demo
  profiles (`Laptop`, `Mac-style`, `Gaming`, `Mac + Cosmic`), or gate the demo
  content behind onboarding as an explicit example. (A fresh install starts
  on an empty Default profile, with the sample profiles seeded alongside it
  as ordinary editable profiles — nothing applies until the user switches to
  one, and user profiles can be renamed in place.)
- [x] **Drop the demo shortcut groups** the same way once real groups can be
  created and kept. (Shortcut groups now start empty; the demo groups were
  removed together with the demo profiles they belonged to.)
- [x] **Remove the demo device entries** once detected keyboards are reliable;
  today they only disappear when the monitor finds real devices. (The device
  picker lists only detected keyboards, with an explicit "No keyboards
  detected" state.)
- [x] **Detect keyboards plugged in after launch.** The monitor reports the
  startup device list and disconnections, but never adds a newly connected
  keyboard. (A scanner thread polls `/dev/input` for new event nodes and
  adopts keyboards among them; a replugged keyboard replaces its stale
  entry, and a selected device scope follows it to the new node.)
- [x] **Recover key testing after applying profiles or remaps.** Track reader
  lifetime so the two-second hotplug scan reopens xremap's recreated virtual
  keyboard even when it reuses the same event path between scans. Wait for
  the old reader to finish before reconnecting and retry unreadable nodes,
  including ones missed at startup. Input during the reconnection gap is
  not captured. Regression tests cover repeated reader exits and tester
  recovery with reused or changed paths.

## 5. Persistence ("Save" iteration)

- [ ] **Save YAML to a user-selected file** via a native file dialog.
- [x] **Persist profiles and mappings between sessions** via cosmic-config
  (`src/config.rs`; the rule model stays the source of truth and YAML remains
  generated output). Shortcut groups are not persisted yet.
- [ ] **Remember UI state** worth keeping across launches (active profile is
  remembered; selected device scope and onboarding-completed flag are not).
- [ ] **Open onboarding on first launch.** The walkthrough currently opens
  only from the menu. Show it for a fresh install and use the persisted
  completion/skipped flag to avoid reopening it on subsequent launches.

## 6. Apply and service management ("Apply" / "Manage" iterations)

> **Current assumption:** xremap is already installed on `$PATH`, registered as
> a systemd *user* unit named `xremap.service`, and the permissions from
> xremap's [running-without-sudo guide](https://github.com/xremap/xremap/blob/master/doc/running_without_sudo.md)
> are in place (user in the `input` group for `/dev/input` access, a udev rule
> granting the `input` group access to `/dev/uinput`, and the `uinput` module
> loaded). Keyloom talks to the unit through `systemctl --user`
> (`src/service.rs`). Removing these assumptions is tracked in
> [§9](#9-installation-and-distribution-support-long-term).

- [x] **Show the service status** without blocking any editing when it is
  absent. (A header chip shows the state in deliberately generic wording —
  Remapping Enabled / Off / Failed / not set up / Unavailable, plus a
  transient Applying Remaps state while a change makes its way to the
  service — queried at startup and after an apply attempt finishes. The chip
  is passive, with a standard arrow cursor and no click action; xremap is never
  named in the status chip. It reflects the unit, not yet whether an xremap binary
  exists at all.)
- [x] **Write the generated config to the user's xremap config path** with
  validation and clear failure feedback. (Written to
  `$XDG_CONFIG_HOME/xremap/keyloom.yml`; hand-written files are backed up
  before the first overwrite, and write failures surface as a toast.)
- [x] **Reload/apply on demand** so saved changes take effect. (Changes apply
  themselves: every change that actually rewrites the config schedules a
  debounced restart of the `xremap.service` user unit, paced to stay under
  systemd's start rate limit. Success is silent — the change's own toast is
  the confirmation — and failures surface the systemctl error as a toast.
  There is deliberately no Apply button.)
- [ ] **Full service controls:** explicit start, stop, and enable/disable on
  login, beyond the restart that Apply performs.
- [ ] **Point the unit at the generated config.** Auto-apply restarts
  whatever `ExecStart` the unit has; verify (or help fix) that the unit
  actually reads `$XDG_CONFIG_HOME/xremap/keyloom.yml` instead of some other
  file. A unit running with `--watch=config` would even make restarts
  unnecessary.
- [ ] **Permission guidance.** Detect missing `/dev/input` read access (the
  `input` group) and walk the user through fixing it instead of failing
  silently. Per xremap's running-without-sudo guide this also covers write
  access to `/dev/uinput` (udev rule for the `input` group) and the `uinput`
  kernel module being loaded.

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

- [x] **Keep custom dialogs open when clicking inside them.** The shared
  modal card captures clicks on text, padding, and unused space so they
  cannot reach the dismissal backdrop; controls and outside clicks still work.
- [x] **Real About dialog** replacing the toast, with the embedded SVG logo,
  Cargo build version, application description, and technology credits.
  Uses the same modal chrome as onboarding; Close, Escape, and backdrop
  clicks dismiss it.
- [x] **Add license information to About** for GPLv3-only (`GPL-3.0-only`),
  with the license identifier from Cargo metadata and a no-warranty notice.
- [ ] **Desktop entry and icon** in `data/` so the app installs and launches
  from a menu. The embedded logo in `assets/keyloom_logo.svg` is available,
  but desktop integration and an installed application icon are still missing.
- [x] **Choose a license** — GPLv3-only (`GPL-3.0-only`), with the full text
  in [LICENSE](../LICENSE), Cargo package metadata, and a README license summary.

## 9. Installation and distribution support (long term)

Removing the assumptions listed in [§6](#6-apply-and-service-management-apply--manage-iterations):
today Keyloom requires a preinstalled xremap on `$PATH`, a registered
`xremap.service` systemd user unit, and manually configured permissions.

- [ ] **Detect an xremap installation** (binary on `$PATH`, its version) as
  distinct from the service unit's state, without blocking any editing when
  it is absent.
- [ ] **Per-distribution dependency strategy.** Decide how xremap gets onto
  each supported distro: declare it as a package dependency where a package
  exists (e.g. Fedora copr, AUR, Gentoo guru), bundle/ship our own copy where
  none does, or install via `cargo install` as a fallback. Track this per
  packaging target (deb for Pop!_OS/Ubuntu first, Flatpak needs its own
  answer since a sandboxed app cannot manage host services directly).
- [ ] **Service-manager abstraction.** `src/service.rs` shells out to
  `systemctl --user` today; keep status/restart behind one interface so other
  supervision schemes (system-level systemd unit, runit/OpenRC, plain
  desktop-autostart process) can slot in per distro. Also make the unit name
  configurable instead of the hardcoded `xremap.service`.
- [ ] **First-time setup wizard.** Guided flow that takes a machine from
  nothing to a working setup: install xremap (per the strategy above), add
  the user to the `input` group, install the udev rule granting the `input`
  group access to `/dev/uinput`, ensure the `uinput` module loads at boot,
  then register and enable an `xremap.service` user unit pointing at the
  generated config (steps from xremap's
  [running-without-sudo guide](https://github.com/xremap/xremap/blob/master/doc/running_without_sudo.md)).
  Needs privilege escalation (polkit/pkexec) for the group, udev, and module
  steps, and must explain the keylogging implication of joining `input`.

## 10. Distant-future possibilities (unscheduled)

These are known limitations we may revisit someday. They are outside the
initial and near-term release scope, are not release blockers, and have no
target release or implementation commitment.

- [ ] **Investigate remote input that bypasses evdev, such as Deskflow's
  Wayland/libei input on the receiving computer.** The remote keyboard is
  absent from the device list, its keystrokes are unavailable to the tester
  and recording modes, and xremap cannot remap those events. See
  [Current_Features.md](Current_Features.md#remote-input-limitation).
  A possible focused-window tester mode would only display received keys;
  it would not enable remapping or identify the remote physical keyboard.
  Actual remapping support would need investigation of a different input
  path or backend integration. No approach has been selected or implemented.

# Functionality TODO

Tracks remaining work for Keyloom. See [Current Features](Current_Features.md)
for implemented behavior and [Upcoming Features](Upcoming_Features.md) for
features deferred beyond the first public release.

## 0.1.0 release commitments

The first public release assumes **xremap is already installed** on a system
using systemd user services. Keyloom will guide the user from that prerequisite
to working remapping through first-run setup. Installing or bundling xremap is
reserved for a later release.

The following are release blockers, with detailed tasks below:

- First-run setup and remembered completion state (§5 and §9).
- A user-level service reading Keyloom's configuration and working input
  permissions (§6 and §9).
- Persisted, working application-specific remaps and their shortcut rules (§7).
- Persisted, working layers (§7).

Unchecked tasks outside these commitments are not automatically release
blockers. Checked items describe completed work; planned capabilities below
must not be read as already implemented.

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
  size; the ISO variant is auto-detected per keyboard from evdev capabilities.
  The pre-redesign language charmaps and typed-text preview were dropped
  with the old layout engine.)
- [x] **Preserve mappings across layout/form-factor changes.** Mappings key off
  physical identity, so switching the displayed deck must not change or delete
  rules; verified with a regression test (including unchanged generated YAML).
- [x] **Follow the selected keyboard's form factor.** Keyboard and Tester
  use the selected device's guess, with persistent size and ANSI/ISO overrides
  per keyboard and separately for "All keyboards". The detected size and
  variant are selected by default in the Size picker. Reconnects
  restore choices using the best available device identity. See
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

- [x] **Explain keyboards that remapping holds.** A running remapper grabs the
  keyboards it manages, and their keys then reach it alone: the selected
  keyboard looked connected but registered nothing in the tester, with no way
  to tell that apart from a broken device. The monitor now reports which input
  nodes the remapper holds (read from its open descriptors, so no keyboard is
  ever grabbed to find out), the device picker marks them along with the
  remapper's own output keyboard, and the tester names the situation and points
  at the pause control. Not covered: a remapper running as another user, whose
  descriptors are unreadable, and remappers other than xremap.
- [x] **Let the tester see a held keyboard's own keys.** The header's status
  chip stops and starts remapping, and is the only thing that does: a pause
  survives view switches and closing the app, so nothing silently takes the
  keyboard back mid-test, and the chip starts the unit again whether or not
  this session is what stopped it. Mapping changes never start a paused
  service, because starting it reads the config as it stands anyway.

## 5. Persistence ("Save" iteration)

- [x] **Persist profiles and mappings between sessions** via cosmic-config
  (`src/config.rs`; the rule model stays the source of truth and YAML remains
  generated output). Shortcut groups are not persisted yet.
- [ ] **Remember selected device scope** across launches. The active profile
  is already remembered; setup completion is covered by the next task.
- [ ] **First-run setup lifecycle (0.1.0 blocker).** Open the setup flow on
  first launch, persist completion or deferral, and allow reopening it from
  the menu. Resume incomplete setup without claiming the system is ready.
  The current menu-only walkthrough does not perform the system setup in §9.

## 6. Apply and service management ("Apply" / "Manage" iterations)

> **Current assumption:** xremap is already installed on `$PATH`, registered as
> a systemd *user* unit named `xremap.service`, and the permissions from
> xremap's [running-without-sudo guide](https://github.com/xremap/xremap/blob/master/doc/running_without_sudo.md)
> are in place (user in the `input` group for `/dev/input` access, a udev rule
> granting the `input` group access to `/dev/uinput`, and the `uinput` module
> loaded). Keyloom talks to the unit through `systemctl --user`
> (`src/service.rs`). Removing these assumptions is tracked in
> [§9](#9-first-run-system-setup-010-blocker). For 0.1.0, only the
> preinstalled xremap prerequisite remains the user's responsibility.

- [x] **Show the service status** without blocking any editing when it is
  absent. (A header chip shows the state in deliberately generic wording —
  Remapping Enabled / Paused / Failed / not set up / Unavailable, plus a
  transient Applying Remaps state while a change makes its way to the
  service — queried at startup and after an apply or a switch finishes.
  xremap is never named in the status chip. It reflects the unit, not yet
  whether an xremap binary exists at all. A stopped unit is simply Paused,
  however it came to be stopped; the chip doubles as the control for that,
  and stays passive only when there is no unit to act on or a restart is in
  flight.)
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
- [ ] **Enable or disable the unit on login**, the one service control the
  status chip does not cover now that it starts and stops the unit
  (`src/service.rs`).
- [ ] **Point the unit at the generated config (0.1.0 blocker).** Auto-apply restarts
  whatever `ExecStart` the unit has; verify (or help fix) that the unit
  actually reads `$XDG_CONFIG_HOME/xremap/keyloom.yml` instead of some other
  file. A unit running with `--watch=config` would even make restarts
  unnecessary.
- [ ] **Permission guidance (0.1.0 blocker).** Detect missing `/dev/input` read access (the
  `input` group) and walk the user through fixing it instead of failing
  silently. Per xremap's running-without-sudo guide this also covers write
  access to `/dev/uinput` (udev rule for the `input` group) and the `uinput`
  kernel module being loaded.

## 7. Advanced remapping in generated config ("Expand" iteration)

Everything here is already editable in the UI; the unchecked items are
still only previewed in memory. The tasks below are 0.1.0 blockers.

- [x] **Tap/hold mappings** → xremap `held`/`alone` output.
- [x] **Two-way swaps** → two explicit generated entries.
- [x] **Disabled keys** → generated no-op mapping (empty output list).
- [x] **Device-scoped mappings** → per-device `modmap` sections keyed to the
  selected keyboard.
- [ ] **Persist shortcut groups.** Save and restore groups, their rules, and
  application scopes with profiles so remaps survive relaunch.
- [ ] **Working shortcut groups (chord → chord rules).** Generate xremap
  `keymap` blocks, including the "any modifier" matching option, and apply
  edits through the existing automatic configuration workflow.
- [ ] **Application-specific remaps.** Provide an application picker and
  generate application filters so rules affect only the selected applications.
  Verify matching on the supported desktop environments and explain any
  unsupported setup. Confirm a rule works in its target app and does not
  affect another app, including after relaunch.
- [ ] **Working layers.** Turn the previewed Caps-Lock navigation layer into
  generated layer configuration so holding the layer key actually changes
  what the other keys do. Persist layer configuration with profiles and
  apply edits automatically. Verify entering and leaving a layer, restoring
  normal keys on release, and retaining it after relaunch. The current layer
  is a visual preview only.

## 8. App polish and distribution

- [x] **Keep the application frame symmetric without a navigation rail.**
  Keyloom uses its own header tabs, so the unused libcosmic navigation rail is
  explicitly closed and the framed content receives matching left and right
  window insets.
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

## 9. First-run system setup (0.1.0 blocker)

Today the user must configure the service and permissions outside Keyloom.
For 0.1.0, first-run setup must handle or guide these steps with an already
installed xremap. This extends the existing onboarding walkthrough.

- [ ] **Check the prerequisite.** Detect the installed xremap binary and
  version. Explain when it is missing or incompatible without blocking
  mouse-driven editing. Do not install or bundle xremap in this release.
- [ ] **Install the user-level systemd unit.** Create an `xremap.service`
  user unit pointing at the installed binary and Keyloom's generated config,
  reload the user service manager, and start remapping once setup is ready.
  Verify an existing unit before reusing or changing it; preserve unrelated
  user configuration. Coordinate config-path validation with §6.
- [ ] **Guide input-group membership.** Check whether the current user has
  the required group access and walk them through joining the `input` group.
  Explain the access being granted and any logout/login needed before it
  takes effect; recheck effective access before declaring setup complete.
- [ ] **Install the udev rule and prepare uinput.** Install the rule needed
  for input access, reload rules as needed, and ensure `/dev/uinput` is
  available with the required permissions, including module loading when
  necessary. Explain any remaining reconnect or session-restart steps.
- [ ] **Request system authorization graphically.** Use the desktop's
  authentication prompt (for example, a polkit-backed helper) for privileged
  system changes such as installing the udev rule. Keep the main app
  unprivileged and handle canceled or failed authorization with a retry path.
- [ ] **Verify setup end to end.** Confirm effective input access, a running
  user service using Keyloom's config, and a working sample remap. Reopening
  setup must safely reuse completed steps and report unresolved failures.

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

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
- [x] **First-run setup lifecycle (0.1.0 blocker).** Setup opens on the first
  launch and from the menu (or the header chip while nothing is set up); the
  stored state records completion when every step is in order or only waits
  for a login, and deferral otherwise, so it never reopens on its own. Every
  opening re-runs the checks, so a resumed setup shows what is actually done
  rather than assuming it. The old tutorial's Caps Lock → Escape example
  lives on the summary page.

## 6. Apply and service management ("Apply" / "Manage" iterations)

> **Current assumption:** xremap is already installed on `$PATH`. First-run
> setup ([§9](#9-first-run-system-setup-010-blocker), `src/setup.rs`) takes
> care of the rest from xremap's
> [running-without-sudo guide](https://github.com/xremap/xremap/blob/master/doc/running_without_sudo.md):
> the `input` group for `/dev/input` access, a udev rule granting the group
> access to `/dev/uinput` with the `uinput` module loaded, and a systemd
> *user* unit named `xremap.service` reading the generated config. Keyloom
> talks to the unit through `systemctl --user` (`src/service.rs`).

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
- [ ] **Disable the unit on login.** Setup enables it; the status chip only
  starts and stops it, and nothing turns the login start off again
  (`src/service.rs`).
- [x] **Point the unit at the generated config (0.1.0 blocker).** Setup
  installs a unit whose `ExecStart` runs the found binary on
  `$XDG_CONFIG_HOME/xremap/keyloom.yml`, and inspects an existing unit
  first: one that names `keyloom.yml` is reused, one that does not is shown
  with its `ExecStart` and can be replaced (backed up) or kept. Auto-apply
  still restarts whatever unit is there; a kept foreign unit means Keyloom's
  remaps have no effect, which only setup points out (see §9 follow-ups).
- [x] **Permission guidance (0.1.0 blocker).** Setup checks effective
  membership in the `input` group and whether `/dev/uinput` opens for
  writing, explains what is missing, and fixes each through the desktop's
  authentication prompt (see §9).

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

- [x] **Check the prerequisite.** The xremap binary is looked up on `PATH`
  and its version shown; a missing binary is explained and editing keeps
  working. Compatibility is not checked beyond the binary answering
  `--version`.
- [x] **Install the user-level systemd unit.** Setup writes
  `~/.config/systemd/user/xremap.service` (marked as Keyloom's, so its own
  file is rewritten freely and anyone else's is backed up beside it first),
  reloads the user manager, enables the unit, and starts it once access is
  in effect. An existing unit is inspected before anything is touched (§6).
- [x] **Guide input-group membership.** Membership in effect (this
  process's groups) is told apart from membership on record (`/etc/group`),
  so the step explains the pending logout/login; the fix runs `usermod -aG
  input` for the user and explains the access it grants. Only a local
  `input` group is checked; a system without one is reported, not fixed.
- [x] **Install the udev rule and prepare uinput.** The step opens
  `/dev/uinput` for writing to know where it stands; the fix installs
  `00-xremap-input.rules` under `/etc/udev/rules.d` (the packaged rule under
  `/usr/lib` is recognized), loads `uinput` and registers it in
  `modules-load.d`, reloads udev, and re-triggers the device. A rule that is
  installed but not in effect points at the next login.
- [x] **Request system authorization graphically.** Privileged changes run
  through `pkexec`; a dismissed or refused prompt is reported on the step
  with the fix still on offer, and a missing `pkexec` shows the commands to
  run by hand. The prompt is polkit's generic one (it names `usermod` or
  `/bin/sh`); a packaged polkit action or helper would let it name Keyloom.
- [x] **Verify setup end to end.** Reopening setup re-runs every check, so
  completed steps are reused from what the system says rather than from
  memory, and the summary reports what is unresolved. Effective input access
  and a running, enabled service on Keyloom's config are verified; the
  sample remap is only applied (see below).
- [ ] **Verify a sample remap end to end.** After the Caps Lock → Escape
  example, observe the remapper's output device to confirm the remap works
  instead of trusting that the service started.
- [ ] **Merge into an existing unit.** A unit the user wrote can only be
  replaced or kept; xremap merges several config files, so offering to add
  `keyloom.yml` to its `ExecStart` would keep their setup working. Until
  then, a kept foreign unit leaves the header saying remapping is enabled
  while Keyloom's remaps are ignored.
- [ ] **Notice a remapper running as another user.** A system-wide xremap
  (or one started by hand as root) competes for the same keyboards; setup
  does not look for one.

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

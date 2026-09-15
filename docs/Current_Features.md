# Current Features

What Keyloom does today. This is the counterpart to
[Functionality_TODO.md](Functionality_TODO.md), which tracks what is still
missing and defines the 0.1.0 release commitments.
[Upcoming Features](Upcoming_Features.md) tracks plans beyond that release.

**The big caveat:** shortcut groups are an in-memory preview and live only
for the current session. Profiles and key mappings, however, are persisted
via cosmic-config and translated into an xremap configuration file at
`~/.config/xremap/keyloom.yml` on every change (see "Persistence and
generated configuration" below). Keyloom applies changes by restarting the
`xremap.service` systemd user unit automatically; first-run setup installs
that unit and walks through the input permissions (see "First-run setup"
below). Installing xremap itself is still up to the user.

## Application shell

- Native Rust + libcosmic application; runs on Wayland and X11.
- Follows the system light/dark theme.
- Header with brand, profile switcher, view tabs (Keyboard / Tester /
  Shortcuts), remapping status, and access to setup, reset, and About actions.
- First-run setup opens on its own the first time Keyloom runs and can be
  reopened from "Set up remapping" in the menu, or from the header's status
  chip while no service is set up (see "First-run setup" below).
- About dialog with the application version, description, project credits,
  and GPLv3-only license information.
- Undo is available for destructive or notable changes.

## Keyboard view

- Renders every form factor (100%, 80% TKL, 75%, 65%, 60%) in ANSI and ISO
  assemblies.
- The deck follows the selected keyboard's detected form factor. Manual size
  and ANSI/ISO choices are remembered per keyboard, with independent choices
  for "All keyboards". The Size picker selects the detected size and ANSI/ISO
  variant by default; manual selections change only the display (see
  [Form_Factor_Detection.md](Form_Factor_Detection.md)).
- Physical key presses do not light up the remap deck; live key highlights
  are reserved for Tester. "Record a key" capture still accepts physical input.
- Clicking a key opens the key editor, and the deck displays configured tap
  and hold actions.
- Layer preview: a toggleable Navigation layer shows what H/J/K/L and friends
  become while Caps Lock is held.
- Mapping summary and remaps dialog for reviewing, editing, and removing
  mappings in the active profile. Removal requires confirmation and is
  undoable.
- "Applies to" device scope selector on the toolbar (see Devices below).

## Key editor

- Searchable action catalog with categories: modifiers, navigation, media,
  letters, numbers, numpad, function keys, punctuation, other (including
  Disabled). Modifiers name their side explicitly (Left/Right Control, Shift,
  Alt, Super), and the catalog offers every key the app renders.
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
- Profiles can be created, duplicated, renamed, and deleted. The active profile
  cannot be deleted, so at least one profile always remains; deletion is
  undoable.
- A fresh install seeds an empty Default profile plus editable starter
  profiles (Laptop, Mac-style, Gaming, Media F-row). They are ordinary
  profiles from then on: renamed, edited, and stored like any other.
- "Reset all mappings" asks for confirmation before clearing the active
  profile's mappings. Confirmed resets cannot be undone. Cancel, Escape, or
  clicking outside the dialog keeps the mappings.

## Devices

- Global keyboard monitoring via evdev (`/dev/input/event*`), non-exclusive —
  the compositor still receives every event.
- Detects readable keyboards at startup, distinguishes real keyboards from
  other input devices, and marks keyboards that disconnect.
- A background scan detects keyboards plugged in after launch and reconnects
  devices whose event reader is replaced, including xremap's recreated virtual
  keyboard. Input during the reconnection gap is not captured, and unreadable
  devices are retried later.
- Per-mapping device scope: "All keyboards" or one specific detected keyboard.
- Best-effort form-factor and ANSI/ISO guesses per device from its name and
  reported keys, used for the selected keyboard's deck in both Keyboard and Tester.
  "All keyboards" uses an aggregate guess from connected keyboards (see
  [Form_Factor_Detection.md](Form_Factor_Detection.md)).
- A disconnected selection retains its last known layout. Reconnecting the
  same device restores its selection and saved display choices when its
  identity can be matched. If another keyboard reuses the selected event
  node, the selection returns to "All keyboards".
- Software-created evdev keyboards, such as Solaar and xremap devices, remain
  available for explicit testing and targeting. When physical keyboards are
  connected, virtual devices do not influence "All keyboards" layout detection.

### Keyboards held by remapping

While remapping runs, xremap takes exclusive control (`EVIOCGRAB`) of every
keyboard it is configured for and re-emits their keys on a virtual keyboard of
its own. Those keyboards still appear connected and readable, but the kernel
routes their events to xremap alone, so they light up nothing in the tester and
record nothing in the shortcut editor. Keyloom detects which keyboards are held
— by reading which input nodes the running remapper has open, which is
side-effect free — and labels the remapper's own keyboard as the remapped
output in the device picker. Keys observed there have already passed through
remapping, so they report its output rather than the key that was pressed.

Pausing remapping from the header's status chip is the only way to watch a held
keyboard's own keys (see "Tester view" and "Applying"). A remapper running as
another user (a system service rather than the user unit Keyloom manages) hides
which devices it holds; its keyboards stay silent with no explanation.

### Remote input limitation

On a computer receiving Deskflow input through Wayland/libei, remote
keystrokes enter the compositor without passing through `/dev/input/event*`.
The remote keyboard therefore does not appear in Keyloom's device list,
its keys do not register in the tester or recording modes, and the xremap
backend cannot remap those events on the receiving computer. This limitation
applies to input that bypasses evdev; it is not a blanket restriction on all
virtual keyboards or network input. See the
[Deskflow FAQ](https://github.com/deskflow/deskflow/wiki/Project-FAQ),
[libei architecture](https://libinput.pages.freedesktop.org/libei/), and
[xremap architecture](https://github.com/xremap/xremap#concept).

Support is only a distant-future possibility, outside the initial and
near-term release scope, with no target release or commitment. It is tracked
in [Functionality_TODO.md §10](Functionality_TODO.md#10-distant-future-possibilities-unscheduled).

## Tester view

- Keys light up live while held, with mirroring available only in this view.
- Input can be filtered to one monitored keyboard or observed across all
  keyboards. The selection is shared with the editor's device scope, but does
  not alter existing mappings.
- Shows the last observed key, its code, and the keyboard it came from.
- Shows held modifiers and what the last key becomes under the active profile.
- Works before any mapping exists and never modifies mappings — editing
  messages are ignored while the tester is open.
- A keyboard held by remapping is called out instead of leaving the tester
  waiting for keys that cannot arrive, pointing at the header's status chip as
  the way to see its key presses.
- The notice is only about a held keyboard: once remapping is paused, nothing
  is holding anything and the header already says so.

## Shortcuts view

- Shortcut groups per profile, each with a name, enable/disable toggle, an
  application scope label (all applications or named apps), and an
  "any modifier" matching option.
- Rules show modifiers + key → modifiers + key, and the editor can record both
  sides of a chord from a physical keyboard.
- Add group, delete rule, and edit existing rules.
- Groups start empty for every profile; combo mappings made in the key
  editor appear under an automatic "From the keyboard" group.

## Persistence and generated configuration

- Profiles and their mappings (plus the active profile) are stored via
  cosmic-config under `~/.config/cosmic/io.github.blakegardner.Keyloom/`
  and restored on launch; a fresh install seeds the Default profile and
  the starter profiles, which persist like any other from then on.
- Keyboard size and ANSI/ISO overrides also persist across launches,
  independently of profiles. Identity uses the device's model identifiers
  plus its unique identifier, falling back to connection path or name.
  Without a unique identifier, moving ports can require selecting the size
  again; identical devices with matching names and no unique identifier or
  connection information share overrides.
  Display changes do not rewrite the remap configuration or restart xremap.
- Every mapping or profile change regenerates the xremap document from the
  internal rule model and writes it to `$XDG_CONFIG_HOME/xremap/keyloom.yml`
  (usually `~/.config`).
- The generator translates friendly action names into xremap `KEY_*` names,
  emits one `modmap` block for unscoped mappings plus one per device scope
  (`device.only`), renders tap/hold as `held`/`alone`, two-way swaps as two
  entries, and Disabled keys as an empty output (`[]`).
- Files Keyloom generated carry a marker comment; a hand-written xremap
  config found at that path is backed up to `keyloom.yml.bak` before the
  first overwrite, and is never touched just for launching the app.
- Shortcut groups are not yet part of the generated output or the stored
  model.

## First-run setup

Keyloom assumes xremap is already installed; setup takes it from there. A
wizard opens the first time Keyloom runs (and from the ⋯ menu or the
header's status chip afterwards) and walks through four checks, fixing
what it can one step at a time. Keyloom itself stays unprivileged: changes
to the system go through the desktop's authentication prompt (`pkexec`),
one prompt per step. Pages stay high level; paths, the unit, and the exact
commands sit behind a "Show details" toggle that is off by default.

- **xremap** — finds the binary on `PATH` and shows its version. A missing
  xremap is explained, not installed: the explanation links to the project's
  page, which opens in the browser, and editing keeps working.
- **Keyboard access** — checks membership in the `input` group, telling
  membership that is in effect apart from membership that still needs a
  new login. "Add me to the input group" runs `usermod -aG input` and
  explains that any program running as the user gains the same access.
- **Virtual keyboard** — checks that `/dev/uinput` opens for writing. The
  fix installs the rule `KERNEL=="uinput", GROUP="input", TAG+="uaccess"`
  as `/etc/udev/rules.d/00-xremap-input.rules`, loads the `uinput` module
  now and at boot, and reloads udev. A rule the xremap packages ship is
  recognized.
- **Remapping service** — installs Keyloom's own `xremap.service` user unit
  under `~/.config/systemd/user/`, modeled on a hand-written unit proven on
  COSMIC: it waits for the compositor's Wayland socket, runs the found
  binary with `--watch` on `keyloom.yml`, keeps xremap running, and logs at
  info level (xremap's debug level would write every key press to the
  journal). Setup then reloads systemd, enables the unit, and starts it when
  access is already in effect (otherwise it starts at the next login). No
  password is needed. An existing unit is inspected first:
  one that already reads `keyloom.yml` is left alone; one that does not is
  shown with its `ExecStart` and can be replaced (its file is backed up
  beside it) or kept, in which case Keyloom's remaps have no effect.

Every step can be rechecked after manual changes; a cancelled or refused
authorization is reported on the step with the fix still on offer. The
summary page says whether everything works, only a logout and login
remain, or steps still need attention. Setup opens by itself
only on the first launch: closing it records completion when every step is
in order (or only waits for a login) and deferral otherwise, so it never
nags. Only "Skip for now", Finish, or Escape close the wizard; a click
outside it is ignored so a stray click cannot skip setup.

Limitations: the authentication prompt is polkit's generic one, naming
`usermod` or `/bin/sh` rather than Keyloom; a unit the user wrote can only
be replaced or kept, not merged with Keyloom's configuration; an xremap
running as another user (a system service) is not noticed; and no sample
remap is verified end to end.

## Applying (xremap service)

- Changes apply themselves — there is no Apply button. Every change that
  actually rewrites the generated config schedules a restart of the
  `xremap.service` systemd user unit (`systemctl --user restart`) after a
  short debounce, so xremap re-reads the file; restarts are paced to avoid
  systemd's start rate limit, and failures are reported in the application.
- The unit's state is queried at startup (`systemctl --user show`) and shown
  in the header, including active, inactive, failed, missing, unavailable, and
  applying states. An absent or broken service never blocks editing, and
  auto-apply restarts only a unit that is actually running.
- The header's status chip is also the pause control: pressing it stops a
  running unit (`systemctl --user stop`) and starts a stopped or failed one,
  whether or not this session is what stopped it. Remapping is either running
  or paused — a unit Keyloom stopped reads exactly like one that was already
  stopped ("Remapping Paused", amber) — and amber also covers the moments
  between: being paused or resumed, or a change on its way to the service.
  The chip stays passive only while a restart is in flight, or when there is
  no systemd to ask; with no unit at all it opens first-run setup instead.
- The chip has the final say: remapping stopped from it stays stopped —
  switching views, closing Keyloom, and quitting it all leave it alone. Only
  the chip starts it again (or the unit's own start on the next login).
- A mapping change made while remapping is paused is written to the config
  but starts nothing, since a service reads the file when it does start.
- Setup enables the unit at login; there is no separate control to turn
  that off afterwards.
- A unit the user wrote must read `keyloom.yml` itself: setup points this
  out and offers to replace it, but never edits its `ExecStart`.
- Keyloom assumes xremap is installed; the unit and permissions are handled
  by first-run setup (see above).

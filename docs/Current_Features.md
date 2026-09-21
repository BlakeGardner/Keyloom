# Current Features

What Keyloom does today, including the limitations users live with.
[Upcoming Features](Upcoming_Features.md) tracks capabilities that are
planned but not built, and [Technical Backlog](Technical_Backlog.md) tracks
engineering work without a user-facing feature of its own.

Profiles with their key mappings, layers, application scopes, and shortcut
groups are persisted via cosmic-config and translated into an xremap
configuration file at `~/.config/xremap/keyloom.yml` on every change (see
"Persistence and generated configuration" below). Keyloom applies changes
by restarting the `xremap.service` systemd user unit automatically;
first-run setup installs that unit and walks through the input permissions
(see "First-run setup" below), and downloads xremap itself into the user's
`~/.local/bin` when none is installed.

## Application shell

- Native Rust + libcosmic application; runs on Wayland and X11.
- Menu launcher and application icon assets live in `data/` and must be
  installed alongside the binary for desktop integration. How the packages
  install them is described in
  [packaging/README.md](../packaging/README.md).
- Debian, Ubuntu, and Fedora packages are built on the Open Build Service
  from each GitHub release: Debian 13, testing, and unstable, Ubuntu
  24.04, 25.10, and 26.04, and Fedora 43, 44, and Rawhide, for x86_64 and
  aarch64. A distribution built on one of these uses its repository:
  Pop!_OS 24.04 installs the Ubuntu 24.04 packages. Users install either
  from the apt or dnf repository the build service publishes, which also
  delivers updates, or as a one-off `.deb` or `.rpm` attached to the
  release; the [README](../README.md) gives the commands per
  distribution. On Arch Linux and derivatives such as Omarchy, the
  PKGBUILD in `packaging/arch/` builds the latest release from source
  with `makepkg`; it is not on the AUR yet, and no binary pacman packages
  are built. See
  [packaging/README.md](../packaging/README.md). Other package formats are
  not produced.
- Follows the desktop's light/dark preference: directly on COSMIC, and
  through the XDG settings portal on other desktops (GNOME, KDE, and any
  compositor with a portal backend), updating live when it changes.
- On COSMIC the interface adopts the user's accent color from COSMIC
  Settings; on other desktops Keyloom uses its own green accent.
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
- Layers: hold one key to give other keys a second job, edited right on the
  deck (see "Layers" below).
- Applications: keys can act differently while one application is in
  front, edited on the deck the same way (see "Applications" below).
- The deck always shows what applies in the shown scope: the selected
  keyboard (or all keyboards) in the shown application scope (or every
  application). A mapping inherited from a more general scope is drawn
  faded, a key kept normal in this scope says so on the cap, and a key
  that is mapped differently in some other scope carries a small dot.
- Mapping summary and remaps dialog for reviewing, editing, and removing
  mappings in the active profile, each with the keyboard and application it
  applies to. Selecting a row opens the mapping in its own scope. Removal
  requires confirmation and is undoable.
- Keys no deck draws can be remapped too: media and brightness keys, and
  the Mission Control and Launchpad keys of Apple keyboards. "Remap a key
  that isn't shown" in the remaps dialog listens for the key and opens the
  key editor for it; such mappings appear in the remaps list, the keys can
  start a shortcut chord, and the tester names them.
- "Applies to" device scope selector on the toolbar (see Devices below).

## Key editor

- Searchable action catalog with categories: modifiers, navigation, media
  (including Mission Control and Launchpad), letters, numbers, numpad,
  function keys, punctuation, other (including Disabled). Modifiers name
  their side explicitly (Left/Right Control, Shift, Alt, Super), and the
  catalog offers every key the app knows.
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
- "Restore original key" clears the mapping. While a keyboard or an
  application scope is shown, the button becomes "Same as all keyboards" or
  "Same as all applications" and removes the key's own mapping in that
  scope, and a "Normal key here" control keeps the key as it is there
  instead of the remap it would inherit (see "Applications" below).
- Existing combos for the key are listed and removable inline; while an
  application scope is shown, only the combos that apply there are listed.

## Layers

- A layer is a key that, while held, gives other keys a different action:
  the classic example is Caps Lock turning H/J/K/L into arrows. Each profile
  can have up to ten layers, each with a name, the key that holds it, and a
  set of keys with a job in that state. Keys without a job keep working
  normally, and modifiers held at the same time still apply (Shift with a
  layer arrow selects text).
- The Layers picker on the keyboard toolbar switches the deck between the
  normal keys and each layer. In a layer, the deck shows the key that holds it
  and every key's job; keys that cannot take a job are dimmed.
- "+ New layer" creates a layer and asks for its key: click it on the deck or
  press it on a keyboard, with Escape to cancel. The bar above the deck then
  offers changing the key, renaming, and deleting the layer (with
  confirmation, undoable).
- With a layer shown, clicking a key opens the key editor for that key's job
  in the layer, with the same catalog, search, and key recording as normal
  mappings; "Back to normal in this layer" removes the job. Jobs follow the
  "Applies to" device scope and are single actions (no tap/hold, chords, or
  swaps inside a layer).
- The layer key keeps its tap action if it has one (in the starter profile,
  Caps Lock still taps Escape) and does nothing when tapped otherwise. Taking
  on a layer replaces any hold action the key had, and a layer key cannot take
  a job in any layer. Shift, Control, Alt, and Super cannot take jobs either.
- Layers are stored with their profile and applied through the generated
  configuration like mappings (see "Persistence and generated configuration").

## Applications

- An application scope is one application, or a few (a "Terminals" scope
  can hold two terminals), whose keys can differ from every other
  application's. A key without a mapping in the scope keeps its
  all-applications behavior, shown faded on the deck; mapping it there
  starts from that behavior, so changing the tap in one application keeps
  the key's hold action. Keys keep working normally in every other
  application.
- The Applications picker on the keyboard toolbar sits beside Layers and
  switches the deck between every application and each scope of the
  profile, with a count of what each holds. "+ Add application" opens the
  application picker; the bar above the deck then offers changing the
  scope's applications, renaming, and removing the scope (with
  confirmation, undoable; its remaps and shortcuts go with it).
- The application picker lists the applications open right now first,
  named exactly as remapping sees them (Keyloom asks the same xremap the
  service runs, told about the same desktop, with `--list-windows`, which
  exits before it touches any input device),
  then the installed applications from their desktop entries with their
  icons, with a search box, and a field to type an application id for
  anything not listed. An application belongs to at most one scope per
  profile. Installed entries also match their desktop entry's
  `StartupWMClass`, which names the windows of applications running through
  XWayland.
- "Normal key here" in the key editor keeps the key as it is in the shown
  scope, standing in for the remap it would inherit: the way to say "Caps
  Lock is Escape everywhere except in this editor". It also keeps a layer
  the key holds from activating in that application.
- Precedence when several mappings of a key apply: the most specific wins.
  An application's mapping beats a keyboard's (an exception made for an
  application holds on every keyboard), and both beat the general one; a
  mapping for one keyboard in one application beats all of them. Removing a
  key's own mapping in a scope lets the next more general one apply again.
- One context at a time: showing an application scope leaves the layer
  and the other way round. Layer jobs behave the same in every application.
- Application ids are matched exactly (case included), the way xremap
  matches them. Matching needs an xremap build with a client for this
  desktop (the `full` build Keyloom downloads has one for every desktop;
  GNOME's Wayland session also needs xremap's GNOME Shell extension).
  Setup's xremap step says whether this build can tell which window is in
  front here, and the picker explains when it could not ask which
  applications are open.

## Profiles

- Multiple named profiles, each with its own mappings, layers, application
  scopes, and shortcut groups.
- Profiles can be created, duplicated, renamed, and deleted. The active profile
  cannot be deleted, so at least one profile always remains; deletion is
  undoable.
- A fresh install seeds an empty Default profile plus editable starter
  profiles (Laptop, Mac-style, Gaming, Media F-row, Navigation layer, which
  holds a Caps Lock navigation layer, and Shortcut examples). They are
  ordinary profiles from then on: renamed, edited, and stored like any other.
- Shortcut examples is the small starter made of shortcut groups: Super
  plus C, V, and Z send copy, paste, and undo everywhere, and a Terminals
  application scope covering the common terminals sends their Ctrl+Shift
  copy and paste instead, so Ctrl+C keeps interrupting. It shows a chord
  rule, an application scope, and which of the two wins.
- "Reset all mappings" asks for confirmation before clearing the active
  profile's mappings, layers, application scopes, and shortcut groups.
  Confirmed resets cannot be undone. Cancel, Escape, or clicking outside the
  dialog keeps them.

## Devices

- Global keyboard monitoring via evdev (`/dev/input/event*`), non-exclusive —
  the compositor still receives every event.
- Detects readable keyboards at startup, distinguishes real keyboards from
  other input devices, and marks keyboards that disconnect.
- A background scan detects keyboards plugged in after launch and reconnects
  devices whose event reader is replaced, including xremap's recreated virtual
  keyboard. Input during the reconnection gap is not captured, and unreadable
  devices are retried later.
- Per-mapping device scope: "All keyboards" or one specific detected
  keyboard. A key can be mapped for all keyboards and differently for one
  keyboard; with that keyboard selected in "Applies to", the deck shows its
  own mapping, or the all-keyboards one faded, and edits made there apply to
  that keyboard alone.
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

Support is only a distant-future possibility, with no target release or
commitment. What an approach would have to solve is noted in
[Technical Backlog](Technical_Backlog.md#input-paths-beyond-evdev).

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
  application scope (every application, or one of the profile's application
  scopes, chosen from the chip on the group; "New application…" opens the
  application picker), and an "any modifier" matching option.
- Groups are listed in sections per application scope, in the order their
  rules apply: each scope of the profile, then every application.
- Rules show modifiers + key → modifiers + key, and the editor can record both
  sides of a chord from a physical keyboard. Rules with both sides recorded
  apply; incomplete rules and paused groups generate nothing.
- A recorded modifier matches either key of its pair. Clicking a modifier
  in the editor's "When I press" chord narrows it to the left or right key
  alone (shown as "L Ctrl" or "R Ctrl"), for setups that give the two keys
  different jobs, such as a Command key remapped to Right Control.
- Add group, delete rule, and edit existing rules. Groups and rules are
  stored with the profile and applied like mappings.
- Groups start empty in every starter profile except Shortcut examples; combo
  mappings made in the key editor appear under an automatic "From the
  keyboard" group, one for every application and one per application scope
  shown while the combo was made.
- "Any modifier" makes a group's shortcuts match while Shift, Ctrl, Alt, or
  Super is held as well, releasing that modifier around the output; with two
  unrelated modifiers held, only one of them is released. A rule from a key
  to itself, such as Volume Up to Volume Up, adds only those entries, so the
  plain key keeps working as it is.

## Persistence and generated configuration

- Profiles with their mappings, layers, application scopes, and shortcut
  groups (plus the active profile) are stored via cosmic-config under
  `~/.config/cosmic/io.github.blakegardner.Keyloom/` and restored on launch;
  a fresh install seeds the Default profile and the starter profiles, which
  persist like any other from then on. Profiles saved before layers,
  application scopes, or stored shortcut groups existed load with none, and
  their mappings apply in every application.
- Keyboard size and ANSI/ISO overrides also persist across launches,
  independently of profiles. Identity uses the device's model identifiers
  plus its unique identifier, falling back to connection path or name.
  Without a unique identifier, moving ports can require selecting the size
  again; identical devices with matching names and no unique identifier or
  connection information share overrides.
  Display changes do not rewrite the remap configuration or restart xremap.
- Every mapping, layer, application scope, shortcut group, or profile
  change regenerates the xremap document from the internal rule model and
  writes it to `$XDG_CONFIG_HOME/xremap/keyloom.yml` (usually `~/.config`).
- The generator translates friendly action names into xremap `KEY_*` names
  and emits one `modmap` block per scope: filtered on the application scope's
  ids (`application.only`), on the keyboard (`device.only`), on both, or on
  neither. Blocks come most specific first, in the precedence order above,
  because xremap uses the first block that mentions a key and whose filters
  match. Tap/hold renders as `held`/`alone`, two-way swaps as two entries,
  Disabled keys as an empty output (`[]`), and "normal here" as the key
  mapped to itself.
- Layers become xremap `virtual_modifiers` plus `keymap` rules. Each layer
  key is remapped to a stand-in virtual modifier (braille-dot key codes,
  which no keyboard emits and xremap never passes on), as the hold side of a
  tap/hold key when the key has a tap action; every job becomes a
  `<modifier>-<key>` rule, scoped per keyboard where the job is. A job on a
  key whose normal press is remapped targets what the key became, since
  xremap applies mappings before rules; a key remapped in one application
  or on one keyboard gets its own rule for that scope.
- Shortcut groups become `keymap` blocks after the layers', groups limited
  to an application first: each complete rule is a `Mods-KEY: Mods-KEY`
  entry, and "any modifier" adds an entry per other modifier held.
- Files Keyloom generated carry a marker comment; a hand-written xremap
  config found at that path is backed up to `keyloom.yml.bak` before the
  first overwrite, and is never touched just for launching the app.

## First-run setup

Setup starts from a system with or without xremap. A wizard opens the first
time Keyloom runs (and from the ⋯ menu or the header's status chip
afterwards) and walks through four checks, fixing what it can one step at a
time. Each step page has one main button, and it does what the step needs:
it carries out the fix and moves on once a fresh check confirms it worked,
or reads "Continue" where the step is already in order or cannot be fixed
from Keyloom. Moving forward passes over steps that are already in order
(or only wait for a login), so setup stops only where something is left to
do; Back still shows every step, and the progress dots mark the ones passed
over. Keyloom itself stays unprivileged: changes to the system go through
the desktop's authentication prompt (`pkexec`), one prompt per step.

Pages stay short. A "Show details" toggle, off by default, says what a step
changes and how to do it by hand, with the commands selectable and
copyable, "Check again" for after manual changes, and "Skip this step" for
moving on without the fix.

- **xremap** — finds the binary on `PATH`, or Keyloom's own download in
  `~/.local/bin`; the details show its version, whether Keyloom downloaded
  it, and whether its build can tell which window is in front on this
  desktop, which application-specific remaps depend on. When those remaps
  need more than setup provides (xremap's GNOME Shell extension on GNOME's
  Wayland session, or a build with a client for this desktop), the summary
  says so, as does the step when it is shown. With no xremap at all,
  "Install xremap" fetches the release Keyloom was tested with (its `full`
  build, with a client for every desktop) from the project's GitHub releases
  into `~/.local/bin`: no administrator access is needed, the download is
  checked against the digest recorded in Keyloom before anything is written,
  and the binary is asked for its version before it is put in place.
  Keyloom's own download is offered an update when Keyloom moves to a newer
  release; a binary the user installed is never touched. Processors without
  an xremap release (anything but x86_64 and aarch64) get an explanation, a
  link to the project page, and "Check again" instead. Editing keeps working
  throughout.
- **Keyboard access** — checks membership in the `input` group, telling
  membership that is in effect apart from membership that is not yet. Setup
  asks for a restart rather than a new login, since the systemd user
  manager that runs xremap can outlive a logout and keep its old groups.
  The fix runs `usermod -aG input`; the details explain that any program
  running as the user gains the same access.
- **Virtual keyboard** — checks that `/dev/uinput` opens for writing. The
  fix installs the rule `KERNEL=="uinput", GROUP="input", TAG+="uaccess"`
  as `/etc/udev/rules.d/00-xremap-input.rules`, loads the `uinput` module
  now and at boot, and reloads udev. A rule the xremap packages ship is
  recognized. An installed rule that is not in effect waits for the restart
  while joining the input group does, and is offered again otherwise.
- **Remapping service** — installs Keyloom's own `xremap.service` user unit
  under `~/.config/systemd/user/`, modeled on a hand-written unit proven on
  COSMIC: on Wayland sessions it waits for the compositor's socket (X11
  sessions skip the wait, since none appears), runs the found binary with
  `--watch` on `keyloom.yml`, names the detected desktop with `--desktop`
  when the binary lists it (so xremap asks the right compositor for
  application-specific rules; older builds, and builds without this
  desktop's client, are left to choose for themselves), keeps xremap
  running, and logs at info level (xremap's debug level would write every
  key press to the journal). Setup then reloads systemd, enables the unit,
  and starts it when access is already in effect (otherwise it starts after
  the restart). No password is needed. To turn it on by hand instead, the
  details save the file (and reload systemd) without enabling or starting
  it, then show the `systemctl --user enable` command that does, with
  `--now` only when access is already in effect. An existing unit is
  inspected first: one that already reads `keyloom.yml` is left alone; one
  that does not is shown with its `ExecStart` and can be replaced (its file
  is backed up beside it) or kept with "Keep mine", in which case Keyloom's
  remaps have no effect unless its command line is given `keyloom.yml` as
  well, as the details explain.

A failed fix is reported on its step with the fix still on offer; any
failure other than a dismissed prompt also opens the details, so the manual
commands are at hand (without `pkexec`, the main button becomes "Check
again"). The summary page says whether everything works, only a restart
remains, or steps still need attention; each step's row reopens that
step, and an unfinished setup offers "Continue setup" back to the first step
the user can still act on. Setup opens by itself only on the first launch:
closing it records completion when every step is in order (or only waits for
a restart) and deferral otherwise, so it never nags. Only "Set up later",
Finish, or Escape close the wizard; a click outside it is ignored so a stray
click cannot skip setup. A page taller than the window scrolls while its
buttons stay in view.

Limitations: the authentication prompt is polkit's generic one, naming
`usermod` or `/bin/sh` rather than Keyloom; a unit the user wrote can only
be replaced or kept, not merged with Keyloom's configuration; an xremap
running as another user (a system service) is not noticed; logging into a
different desktop leaves the unit naming the old one until setup's "Update
remapping" is used; a distribution's xremap older than 0.15.13 gets
neither the download nor `--desktop`; and no sample remap is verified end
to end.

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
- xremap itself, the unit, and the permissions are handled by first-run
  setup (see above), which downloads xremap when none is installed.

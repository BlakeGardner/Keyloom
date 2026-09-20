# Upcoming Features

Capabilities a user would notice, planned but not scheduled. No delivery
dates are assigned, and nothing here is a commitment for a particular
release. See [Current Features](Current_Features.md) for what Keyloom does
today, and [Technical Backlog](Technical_Backlog.md) for engineering work
without a user-facing feature of its own.

## Remapping

- **Richer layer jobs.** Tap/hold and modifier-chord outputs inside a layer,
  and a way to hold a layer with a modifier key without losing the modifier
  (today a modifier used as a layer key stops acting as one).
- **Layers that differ per application.** The model already carries an
  application scope per job, but the deck shows one context at a time;
  letting a layer differ per application needs the deck to show a layer
  inside an application scope.
- **Application differences in the Tester.** "Becomes" reports the
  all-applications outcome only, so an application-specific remap is not
  reflected while testing.
- **A remembered device scope.** The selected device scope is chosen again on
  every launch; the active profile is already remembered.

## Keyboards and interface

- **Mac keyboards on Linux.** Support Mac keyboard layouts and show their
  familiar key names and symbols, including Command and Option, in the
  keyboard view, tester, and key editor. This goes beyond the existing
  Mac-style mapping profiles; it does not imply a macOS version of Keyloom.
- **Non-English keyboards.** Support non-English keyboard layouts with
  matching key labels in the keyboard view, tester, and key editor. Language
  layouts are separate from the existing keyboard sizes and ANSI/ISO physical
  layouts.
- **A translated interface.** Use Fluent for internationalization so
  Keyloom's interface can be translated into other languages. Specific
  translations will be planned separately.
- **A log viewer.** Provide an in-app viewer for remapping service logs so
  users can investigate failures without opening a terminal.

## Setup and the remapping service

- **Undo first-run setup.** Offer an uninstall action that reverses the
  system changes first-run setup made: stop and disable the remapping
  service, remove the user unit Keyloom wrote, delete the xremap Keyloom
  downloaded into `~/.local/bin`, and remove the udev rule and `uinput`
  module configuration it installed with administrator rights. Keyloom should
  undo only what it installed, leaving a distribution's xremap, a unit it did
  not write, and an administrator's own udev rules alone; changes that need
  administrator rights should be summarised before Keyloom asks for them, and
  offered as commands to run by hand when the prompt is unavailable. Removing
  the user's `input` group membership and deciding what happens to saved
  profiles need their own confirmation, since both can affect more than
  Keyloom.
- **Verify a sample remap during setup.** Apply a throwaway remap while
  setting up and observe the remapper's output device to confirm remapping
  works, instead of trusting that the service started.
- **Turning off the login start.** Setup enables the unit, and the status
  chip only starts and stops it; nothing turns the login start off again
  (`src/service.rs`).
- **Following a desktop change.** The unit names the desktop setup saw, so
  logging into another desktop leaves it stale until setup's "Update
  remapping" is used. Keyloom could refresh its own unit at launch when the
  detected desktop or session type changed.
- **Working with a unit the user wrote.** A foreign unit can only be replaced
  or kept; xremap merges several config files, so offering to add
  `keyloom.yml` to its `ExecStart` would keep an existing setup working.
  Until then, a kept foreign unit leaves the header saying remapping is
  enabled while Keyloom's remaps are ignored.
- **Noticing another remapper.** A system-wide xremap, or one started by hand
  as root, competes for the same keyboards; setup does not look for one.
- **Keyloom's xremap beside a single-desktop build.** A distribution's xremap
  without a client for this desktop is used as it is and explained; setup
  could install Keyloom's full build alongside it and prefer it.
- **Broader service-manager support.** Support service managers beyond the
  systemd user-service setup, with configurable service integration where
  needed.

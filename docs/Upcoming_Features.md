# Upcoming Features

These features are planned for after **0.1.0** and do not block the first
public release. No delivery dates are assigned. First-run system setup,
application-specific remaps, and layers are part of the first-release work
tracked in [Functionality TODO](Functionality_TODO.md).

## A translated interface

- [ ] Use Fluent for internationalization so Keyloom's interface can be
  translated into other languages. Specific translations will be planned
  separately.

## Mac keyboards on Linux

- [ ] Support Mac keyboard layouts and show their familiar key names and
  symbols, including Command and Option, in the keyboard view, tester, and
  key editor. This goes beyond the existing Mac-style mapping profiles;
  it does not imply a macOS version of Keyloom.

## Non-English keyboards

- [ ] Support non-English keyboard layouts with matching key labels in the
  keyboard view, tester, and key editor. Language layouts are separate from
  the existing keyboard sizes and ANSI/ISO physical layouts.

## Log viewer

- [ ] Provide an in-app viewer for remapping service logs so users can
  investigate failures without opening a terminal.

## Undo first-run setup

- [ ] Offer an uninstall action that reverses the system changes first-run
  setup made: stop and disable the remapping service, remove the user unit
  Keyloom wrote, delete the xremap Keyloom downloaded into `~/.local/bin`,
  and remove the udev rule and `uinput` module configuration it installed
  with administrator rights. Keyloom should undo only what it installed,
  leaving a distribution's xremap, a unit it did not write, and an
  administrator's own udev rules alone; changes that need administrator
  rights should be summarised before Keyloom asks for them, and offered as
  commands to run by hand when the prompt is unavailable. Removing the
  user's `input` group membership and deciding what happens to saved
  profiles need their own confirmation, since both can affect more than
  Keyloom.

## Broader service-manager support

- [ ] Support service managers beyond the initial systemd user-service setup,
  with configurable service integration where needed.

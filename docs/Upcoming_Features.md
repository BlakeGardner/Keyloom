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

## Ship xremap with Keyloom

- [ ] Bundle a compatible xremap binary or provide it as a package dependency
  for supported distributions. Version 0.1.0 assumes xremap is already
  installed; its first-run setup will configure the service and permissions.

## Broader service-manager support

- [ ] Support service managers beyond the initial systemd user-service setup,
  with configurable service integration where needed.

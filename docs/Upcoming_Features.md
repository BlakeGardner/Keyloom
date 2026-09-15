# Upcoming Features

These features are planned.

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

## Application-specific remaps

- [ ] Let users choose which applications a remap applies to, with an
  application picker instead of manually entered application identifiers.
  For example, use a different shortcut in a terminal than in a browser.
  The current application-specific shortcut controls are session-only
  previews and do not affect live input. Working shortcut groups and their
  persistence are prerequisites for application-specific shortcuts; apply
  application targeting through generated xremap `application` filters.

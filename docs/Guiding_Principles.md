# Guiding Principles

Keyloom's promise is its tagline: **Your keyboard, the way you want it.**
The person at the keyboard decides what each key does, and Keyloom takes
care of everything it takes to make that happen.

These principles shape how Keyloom is designed and built, and how it is
described in the README, on the website, and in the app itself.
[AGENTS.md](../AGENTS.md) carries a short version for contributors and
coding agents; this document is the full statement, and changes start
here.

## Simplicity

Users should be able to do everything through an easy-to-use interface.
Advanced options are there, but they appear only when a user goes looking
for them.

- **Common tasks come first, and take as few steps as possible.**
  Remapping a key is two clicks: the key, then what it should do.
- **Nothing to apply, edit, or run.** Changes save and take effect on
  their own. Nothing a user needs requires a terminal or a configuration
  file.
- **Work it out instead of asking.** Keyloom detects the keyboard's size
  and layout and checks what setup still needs, so the user only makes
  decisions that are really theirs.
- **Progressive disclosure.** Layers, application scopes, technical
  details, and do-it-yourself commands sit one step away (a collapsed
  panel, "Show details"), available to anyone who wants them and out of
  the way for everyone else.
- **One clear next step.** A guided page has one primary button, named
  for what it does, and pressing it does the work.
- **Plain language and sensible defaults.** Name things by what they do
  for the user, not by how they work underneath.
- **Every addition earns its place.** A new control, setting, or step
  needs a clear user need behind it.

## At home on COSMIC, and on every desktop

Keyloom is built with libcosmic and should feel like a first-party COSMIC
application. It must also work well on every major Linux desktop.

- **Follow libcosmic's conventions** and use COSMIC's integrations, such
  as its theme and accent color, where they exist.
- **Prefer shared standards** (XDG portals, desktop entries, systemd user
  services) over anything specific to one desktop.
- **Detect, then fall back.** Optional integrations are detected, and a
  desktop without them gets a usable fallback. Core workflows never
  depend on a COSMIC-only service.
- **Check beyond the development desktop**, and say plainly what could
  not be verified.

## Beautiful and delightful

Visual polish is part of a feature, not something added afterwards.

- **Consistent, responsive, and accessible.** Established libcosmic
  patterns, readable contrast in light and dark, and an interface that
  keeps up with the user.
- **Show the real keyboard.** Keyloom draws the user's keyboard as it
  actually is, so what is on screen matches what is under their hands.
- **Thoughtful feedback.** Every action shows what happened and, when
  something needs attention, what to do next.
- **Purposeful details.** Animation and decoration help people understand
  and enjoy the app; they are never there for their own sake.

## How we talk about Keyloom

The README, the website, the app, and the package descriptions use the
same words:

| | |
|---|---|
| Tagline | Your keyboard, the way you want it. |
| Description | Keyloom is a visual keyboard customization app for Linux. |
| How it works | Click a key, choose what it should do, and it takes effect right away. |
| Simplicity and power | Simple things first. Power when you want it. |

The tagline heads the README, the website (its title, hero, and social
previews), the About dialog, and the first-run setup's welcome page. It
has no variants. Package summaries shorten the description to "Visual
keyboard customization for Linux".

Keyloom is for people who want their keyboard to work their way. They
don't need to know or care which desktop or toolkit Keyloom was built
for; all they need to know is that it does what they need. So the app,
the README, and the website never mention COSMIC, libcosmic, Rust, or
anything else about how Keyloom is built. xremap is the one exception:
it is credited ("Built on xremap"), and setup names it because setup
installs it.

Each principle shows up in the copy as well as in the product:

- **Simplicity:** lead with the everyday task, then reveal the depth.
  Describe outcomes ("Caps Lock becomes Escape in your editor"), not
  mechanisms; xremap, YAML, and systemd belong in the credits and
  technical docs, not the pitch.
- **Every desktop:** this principle shows in the product, not in the
  copy. People should simply find that Keyloom works on their desktop,
  without being told which desktops or toolkits are involved.
- **Beautiful and delightful:** show, don't tell. Let current screenshots
  and recordings of the app carry it, rather than calling Keyloom
  beautiful.
- **Throughout:** short, plain sentences, written to the reader ("your
  keyboard").

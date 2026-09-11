# First Release Scope — Visual Keyboard Remapping

## Release goal

Let a user configure keyboard remappings visually, retain them across sessions,
and have changes apply automatically to an existing xremap setup.

**Click a key → choose its replacement → changes save and apply automatically.**

Keyloom owns the editable rule model, generated configuration, and automatic
service restart. Users remain responsible for installing xremap, configuring
input permissions, and registering an `xremap.service` systemd user unit that
reads the generated configuration. A copy-only YAML workflow is not planned.

This document defines the first-release scope and takes precedence over the
broader [Product_Plan.md](Product_Plan.md). See
[Current_Features.md](Current_Features.md) for implemented behavior and
[Functionality_TODO.md](Functionality_TODO.md) for remaining work.

## Included in v0.1

### 1. Visual editing and review

- Use the Rust + libcosmic application with its keyboard form factors and
  ANSI/ISO assemblies.
- Clicking a key opens its editor with a labeled, searchable output picker,
  including explicit left/right modifiers and keys outside the displayed deck.
- Keep editing usable with a mouse when input devices are unreadable.
- Use physical key identity internally so changing the displayed deck preserves
  mappings.
- Show configured mappings on the keyboard and in the remaps dialog; allow
  selecting an entry to edit it and removing an entry directly.
- Reject no-op self-maps, except when a different hold action makes the self-tap
  meaningful. Multiple sources may share a destination.
- Support the implemented tap/hold, two-way swap, disabled-key, and device-scoped
  mappings in generated configuration.

### 2. Profiles and persistence

- Store named profiles, their key mappings, and the active profile via
  cosmic-config; restore them on launch.
- Start fresh installs on an empty Default profile with editable starter
  profiles available alongside it.
- Support profile creation, duplication, renaming, and deletion, protecting the
  active profile from deletion.
- Provide undo for mapping removal, profile deletion, and resetting mappings.
- Keep shortcut-group persistence and generated chord rules deferred; those
  controls currently provide a session-only preview.

### 3. Generated configuration

- Generate deterministic xremap YAML from the active profile's rule model using
  recognized key names, with stable ordering and no duplicate source entries
  within a generated remap block.
- Write configuration automatically to `$XDG_CONFIG_HOME/xremap/config.yml`
  (normally `~/.config/xremap/config.yml`). No copy or paste step is required.
- Emit unscoped and per-device `modmap` sections as needed; an empty profile
  generates `modmap: []`.
- Mark generated files and back up a foreign configuration before replacing it.
  Do not overwrite a foreign configuration merely because the app launches.
- Surface configuration-write failures clearly.
- Validate generated documents against a recorded xremap release.

The internal rule model is the source of truth. Keyloom does not parse or merge
existing hand-written YAML.

### 4. Automatic apply and service status

- After configuration changes, automatically restart the existing
  `xremap.service` systemd user unit with debounce and restart pacing.
- Show service status in the header, including Applying Remaps while a change
  is pending; allow users to re-check status.
- Surface restart failures and keep editing available when the service is
  missing or unavailable.
- Do not restart the service merely because Keyloom launches.
- Keep installation, permissions, and service registration outside this release's
  managed workflow. Automatic apply assumes the unit reads Keyloom's config.

### 5. Application shell

- Provide Keyboard, Tester, and Shortcuts views, profile switching, and the
  existing onboarding walkthrough available from the menu.
- Use inline modal dialogs for setup, confirmations, remap review, and About.
- Include the embedded logo, build version, description, and credits in About.
  Include the GPLv3-only license information and no-warranty notice in About.

## Explicitly deferred

- Importing or editing existing YAML, exporting to a user-selected file, and
  selecting a different managed configuration path.
- Installing or bundling xremap, permission setup, and registering a user unit.
- Explicit start/stop controls, enable-on-login controls, and alternate service
  managers.
- Persisting shortcut groups and generating chord rules, application filters,
  layers, macros, and key sequences.
- Observed mapped-output verification, expanded diagnostics, and service logs.
- New mouse or additional HID support.

## Acceptance criteria

1. A user can configure Caps Lock → Escape without typing a keycode or editing
   YAML. The mapping is saved and written to the generated configuration.
2. Adding, editing, and removing mappings keeps the keyboard, remaps list,
   persisted model, and generated configuration consistent.
3. Left and right modifiers remain distinct; swaps, tap/hold, disabled keys,
   and device scopes produce valid xremap rules.
4. Changing form factor or ANSI/ISO assembly preserves mapping identities and
   destinations.
5. Profiles and mappings survive relaunch, including the active profile.
6. With a correctly configured existing service, changes apply automatically;
   pending state and failures are visible without an Apply button.
7. Missing xremap service or unreadable input devices does not block mouse-driven
   editing. The UI reports service availability without claiming changes applied.
8. Launching does not overwrite a foreign configuration or restart the service.
   A subsequent edit backs up a foreign configuration before replacing it.
9. An empty profile has a clear empty state and generates a valid empty config.
10. Representative generated documents are checked against the recorded xremap
    version, including simple mappings, swaps, tap/hold, and device scoping.

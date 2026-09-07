# First Release Scope — Visual xremap YAML Generator

## Release goal

Let a user create simple key-to-key remappings visually and copy a valid xremap YAML configuration.

**Click a key → choose its replacement → add the mapping → copy YAML.**

Version 0.1 is a configuration generator. Users manage xremap and apply the generated configuration themselves. Saving configuration files, applying changes, and managing the service belong to later releases.

This document defines the first-release scope and takes precedence over the broader MVP described in [Product_Plan.md](Product_Plan.md). That plan remains the long-term product vision.

## Intended user

A Linux desktop user who wants to generate a few straightforward xremap rules without learning YAML or Linux key names. For this release, the user is responsible for their own xremap installation and configuration workflow.

## Included in v0.1

### 1. Visual source-key selection

- Reuse the existing Rust + libcosmic application and its keyboard layouts and form factors.
- Clicking a key selects it for remapping.
- Show the selected source key clearly, with explicit left/right labels for modifiers.
- Use physical key identity internally; changing displayed layout or form factor must not change or delete existing mappings.
- Keep the complete editing and copying workflow usable with a mouse, without access to input devices.

Existing physical-key highlighting can remain where available, but physical-key capture, device identification, and expanded hardware/layout coverage are not release requirements. The current typed-text preview is not a remapping test; it can be replaced by the mapping editor.

### 2. Simple key-to-key rules

Each rule maps exactly one input key to exactly one output key.

Examples:

- Caps Lock → Escape
- Caps Lock → Left Control
- Right Alt → Right Control
- Left Control → Left Alt, and Left Alt → Left Control

Provide a labeled output-key picker covering the keyboard keys already represented by the app, including keys outside the currently displayed form factor. Users do not enter Linux keycodes or YAML.

Rule behavior:

- Maintain one collection of mappings, with no device or application filters.
- Allow adding, editing, and removing mappings.
- Each source key has at most one mapping. Editing that source replaces its existing destination.
- Multiple source keys may share a destination.
- A swap is represented by two explicit mappings; adding one direction does not create the other.
- Reject mapping a key to itself with a short explanation.
- Keep left and right modifiers distinct.

The generated rules contain no device restrictions. Which keyboards xremap handles remains controlled by the user's xremap setup.

### 3. Mapping review

- Show every configured mapping in a simple list, such as `Caps Lock → Escape`.
- Allow selecting a list entry to edit it and removing an entry directly.
- Keep mappings visible in the list even when their keys are absent from the displayed keyboard.
- Mark configured source keys on the visual keyboard so users can see which keys have rules.
- Use wording such as **Configured mappings**, rather than implying the rules are running.

### 4. YAML preview and copy

- Generate a complete YAML document containing one `modmap` block for the configured rules.
- Update a read-only preview whenever a mapping is added, edited, or removed.
- Provide **Copy YAML**, with success feedback only after the clipboard operation succeeds and a useful error if it fails.
- With no mappings, show an empty-state prompt and disable copying.
- Make the boundary clear in the UI: **Copy this configuration to use with xremap. Changes are not applied by this app.**

Example output:

```yaml
modmap:
  - name: Keyboard mappings
    remap:
      KEY_CAPSLOCK: KEY_ESC
      KEY_RIGHTALT: KEY_RIGHTCTRL
```

The generator uses recognized xremap key names, emits no duplicate source keys, and produces deterministic output for the same mappings. The rule model is the source of truth; the YAML preview is generated from it.

The app does not parse or merge an existing configuration. Copied YAML represents the current collection of mappings only.

### 5. Session-only editing

Mappings live in memory for the current session. There is no draft persistence or file saving in this release. Make that limitation visible near the editor: **Mappings are kept until you close the app. Copy YAML to keep them.**

## Explicitly deferred

- Saving or exporting YAML directly to a file, choosing an xremap config path, and persisting drafts.
- Importing or editing existing YAML.
- Installing, bundling, locating, or invoking xremap from the app.
- Applying configurations, reloading xremap, or starting, stopping, and restarting its service.
- Configuring permissions, elevated helpers, autostart, and background processes.
- Profiles, device-specific rules, and application-specific rules.
- Shortcuts, tap/hold behavior, layers, macros, key sequences, and disabling a key.
- Individual rule enable/disable switches; users can remove rules in v0.1.
- A full event viewer, observed mapped-output verification, service diagnostics, and logs.
- New mouse, media-control, or additional HID support.

## Acceptance criteria

The release is complete when:

1. From a fresh launch, a user can configure Caps Lock → Escape and copy its YAML in approximately 30 seconds without typing a keycode or editing YAML.
2. A user can add a second mapping, change its destination, and remove it; the list, keyboard markers, and YAML preview stay consistent.
3. Left and right modifiers remain separate, and a two-rule modifier swap generates the intended independent entries.
4. Editing a source produces one entry for that source; mapping two sources to one destination is allowed.
5. Changing the displayed keyboard layout or size preserves the physical identities and destinations of existing rules.
6. The copied text matches the preview and is valid YAML using xremap's `modmap` format. Representative generated configurations are checked against a recorded xremap version during development, including a simple remap, a modifier remap, and a swap.
7. The entire generator workflow works when xremap is absent and input devices are unreadable. Neither condition blocks editing or copying.
8. Creating and copying mappings does not change system keyboard behavior, write an xremap configuration file, or interact with a service.
9. An empty collection has a clear prompt, and the UI clearly communicates that mappings are session-only and have not been applied.

## Suggested implementation order

1. Add the minimal source/destination rule model and deterministic YAML generator.
2. Connect visual key selection to an output picker and mapping list.
3. Add configured-key markers, YAML preview, and clipboard copying.
4. Verify the acceptance scenarios and refine empty states and labels.

## Later iterations

1. **Save:** write YAML to a user-selected file and retain editable mappings between sessions.
2. **Apply:** connect the saved configuration to an existing xremap setup, with validation and clear failure feedback.
3. **Manage:** add service status and explicit start, stop, reload, and restart controls.
4. **Expand:** introduce profiles, targeted rules, and advanced remapping behaviors as separate scope decisions.

These are sequencing directions, not requirements for v0.1.

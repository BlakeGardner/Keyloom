# Product Plan — Karabiner-Elements for Modern Linux

## Product vision

Build a polished, visual keyboard remapping application for modern Linux that makes powerful input customization approachable.

The core experience should be:

**See your keyboard → press or click a key → choose what it should do → apply.**

The application will use **xremap as its remapping engine**. The app owns the user experience, configuration model, profiles, device management, application targeting, validation, and service lifecycle, while xremap handles the actual low-level input remapping.

The goal is effectively:

> **Karabiner-Elements for modern Linux, powered by xremap.**

---

# Product principles

### Visual first

The keyboard itself is the configuration interface.

Users should be able to press or click a key and immediately configure it.

### Simple things stay simple

A basic remap such as:

**Caps Lock → Escape**

should take seconds.

Advanced functionality should progressively appear only when needed.

### No config files required

Users should never need to manually edit xremap YAML.

The application generates and manages xremap configuration automatically.

### Power without complexity

The app should expose xremap's power through understandable visual concepts rather than its configuration syntax.

---

# Core user experience

The existing visual keyboard becomes the center of the application.

When the user presses a physical key:

1. The corresponding key lights up on screen.
2. The app identifies the physical key and device.
3. The user clicks the key or selects **Remap**.
4. They choose the new output key or action.
5. The visual keyboard shows the new mapping.
6. The app generates the corresponding xremap configuration.
7. The mapping is applied automatically.

Example:

**Caps Lock → Escape**

The Caps Lock key could visually display:

`Caps`
`→ Esc`

More advanced mappings might appear as:

**Caps Lock**

Tap → Escape  
Hold → Control

The user never needs to know that Linux internally calls these `KEY_CAPSLOCK`, `KEY_ESC`, or `KEY_LEFTCTRL`.

---

# Target users

## Linux desktop users

Users who want straightforward customization:

- Caps Lock → Escape
- Swap Ctrl / Alt / Super
- Mac-style keyboard layouts
- Remap unused keys
- Change media keys
- Fix unusual laptop keyboard layouts

## Power users

Users currently maintaining xremap, keyd, Kanata, KMonad, or similar configurations by hand.

They want the same capabilities with better discoverability and management.

## macOS converts

Users accustomed to Karabiner-Elements who expect keyboard remapping to have a polished graphical interface.

## Developers

Users who want different mappings for terminals, editors, browsers, remote desktop applications, games, or other development tools.

---

# Architecture

## libcosmic application

The main application remains a native **Rust + libcosmic** desktop application.

It is responsible for:

- Visual keyboard rendering
- Input visualization
- Rule editing
- Profiles
- Device selection
- Application selection
- Configuration validation
- xremap configuration generation
- xremap lifecycle management
- Diagnostics
- Settings

---

## Internal configuration model

The application's own data model should be the source of truth.

It should **not** use xremap YAML as its primary internal representation.

Example conceptual rule:

```rust
Mapping {
    input: Key::CapsLock,
    action: Action::TapHold {
        tap: Key::Escape,
        hold: Key::LeftCtrl,
    },
    devices: vec![],
    applications: vec![],
}
```

This allows the app to store additional information that xremap does not need, such as:

- Display names
- Icons
- Rule ordering
- Disabled mappings
- UI state
- Profiles
- Notes
- Migration/version information
- Future backend-independent functionality

---

## xremap configuration generator

A dedicated translation layer converts the application's internal model into valid xremap YAML.

Conceptually:

```text
Visual UI
   ↓
Internal mapping model
   ↓
xremap config generator
   ↓
Generated YAML
   ↓
xremap
   ↓
evdev / uinput
```

Generated YAML should generally be treated as an implementation detail.

An advanced option could allow users to inspect the generated configuration for debugging or learning.

---

# xremap distribution

The preferred default should be to **ship a known-compatible xremap binary with the application**.

This gives the project control over:

- xremap version
- enabled features
- COSMIC compatibility
- configuration syntax
- regressions
- testing

Conceptually:

```text
application/
├── keyboard-mapper
├── libexec/
│   └── xremap
└── licenses/
    └── xremap-MIT.txt
```

The bundled build can initially target COSMIC support.

Later, advanced users and distribution maintainers could optionally select:

```text
Remapping Engine

● Bundled xremap
○ System xremap
```

The bundled version should remain the default supported configuration.

---

# Background service

The application should transparently manage the xremap process.

The user should never need to run:

```bash
sudo xremap config.yml
```

Instead, installation should configure the necessary permissions and service integration once.

The application can then present simple status information such as:

```text
Remapping

✓ Service running
✓ 2 keyboards detected
✓ 7 mappings active
```

The GUI itself should remain unprivileged.

xremap should run through an appropriate service architecture with access to the necessary input devices and `/dev/uinput`.

---

# MVP

The first release should remain deliberately focused.

## 1. Visual keyboard

Use the existing keyboard implementation.

Support common layouts:

- 100%
- TKL / 80%
- 75%
- 70%
- 65%
- 60%

Physical key presses highlight the corresponding visual key.

Keys can also be selected directly with the mouse.

---

## 2. Simple key remapping

Support:

**Input key → Output key**

Examples:

- Caps Lock → Escape
- Right Alt → Right Control
- Menu → Super
- F12 → Play/Pause

Mappings should be visible directly on the keyboard.

---

## 3. xremap integration

The MVP should:

- Generate xremap configuration
- Validate generated configuration
- Start/reload xremap
- Detect xremap failures
- Display useful errors
- Automatically apply configuration changes

Users should never interact with the YAML themselves.

---

## 4. Event viewer

Include a built-in equivalent of Karabiner EventViewer.

When a key is pressed, display:

- Physical device
- Physical key
- Linux keycode
- Current modifiers
- Resulting mapped key

Example:

```text
Keychron Q1

Physical: Caps Lock
Keycode: KEY_CAPSLOCK
Output: Escape
```

This makes the application useful as both a remapper and a keyboard diagnostic tool.

---

## 5. Profiles

Allow users to create configurations such as:

- Default
- Mac-style
- Gaming
- Work
- Laptop
- External keyboard

Profiles should support:

- Enable
- Duplicate
- Rename
- Export
- Import

---

## 6. Device-specific mappings

Allow mappings to target:

**All keyboards**

or

**Specific devices**

Example:

Built-in keyboard:

`Caps → Ctrl`

Apple keyboard:

`Command → Ctrl`

Device selection should use friendly hardware names rather than `/dev/input/eventX`.

---

# Phase 2

## Application-specific mappings

Expose xremap's application-specific functionality through a graphical application picker.

Example:

Global:

`Super+C → Ctrl+C`

Terminal:

`Super+C → Ctrl+Shift+C`

Game:

`Super → Disabled`

Users should select installed or currently running applications rather than entering process identifiers manually.

---

## Tap / hold behavior

Expose advanced xremap mappings visually.

Example:

**Caps Lock**

Tap → Escape  
Hold → Control

Additional behaviors can later include:

- Double tap
- Long press
- Key sequences
- Modifier combinations

---

## Keyboard shortcuts

Allow rules where the input or output consists of multiple keys.

Example:

`Super+C → Ctrl+C`

or:

`Ctrl+Shift+4 → PrintScreen`

The UI should visually render the entire combination.

---

## Rule enable/disable

Each rule should have an individual enable switch.

This allows users to temporarily disable mappings without deleting them.

---

# Phase 3

## Layers

Introduce visual keyboard layers.

Example:

Hold Caps:

```text
H → ←
J → ↓
K → ↑
L → →
```

While configuring a layer, the keyboard visualization should change to show what every key does within that layer.

If feasible within xremap's capabilities, this could become one of the application's strongest differentiators.

---

## Mouse and additional input devices

Expand beyond keyboards.

Potential support:

- Mouse buttons
- Scroll wheel mappings
- Media controls
- Macro pads
- Extra HID buttons

The same interaction model should apply:

**Select input → choose action → apply**

---

# Diagnostics

The application should provide a dedicated diagnostics screen showing:

- xremap status
- xremap version
- Input permissions
- `/dev/uinput` availability
- Detected keyboards
- Generated config location
- Current active profile
- Last xremap error
- Service logs

A user encountering a problem should be able to diagnose most issues without opening a terminal.

---

# Advanced mode

Power users should still have access to deeper information.

Optional features could include:

- Show generated xremap YAML
- Copy generated config
- Show raw Linux keycodes
- Show raw device names
- View xremap logs
- Use system xremap binary
- Configure additional xremap options

These should stay out of the normal workflow.

---

# Product positioning

The product is **not another Linux remapping engine**.

xremap already solves that problem.

The product is the missing graphical layer that turns xremap into a desktop-quality experience.

A useful positioning statement:

> **A visual keyboard customization tool for Linux powered by xremap. Click a key, choose what it should do, and apply it—no config files required.**

The competitive advantage is:

**xremap's capabilities + a genuinely good Linux desktop experience.**

---

# MVP success criteria

A new user should be able to install the application and create:

**Caps Lock → Escape**

within **30 seconds**, without:

- Opening a terminal
- Editing YAML
- Looking up a keycode
- Understanding evdev
- Configuring uinput manually
- Reading xremap documentation

At the same time, the architecture should eventually support:

**Tap Caps → Escape  
Hold Caps → Control  
only on my external keyboard  
except inside VS Code**

entirely through the GUI.

That is the product gap this project should fill.
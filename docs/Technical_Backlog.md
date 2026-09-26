# Technical Backlog

Engineering work that has no user-facing feature of its own: test
infrastructure, packaging, and investigations whose outcome is unknown.
Capabilities a user would notice belong in
[Upcoming Features](Upcoming_Features.md); what Keyloom does today, and the
limitations users live with, are described in
[Current Features](Current_Features.md).

Nothing here is scheduled, and an investigation listed below may conclude
that the work is not worth doing.

## Input paths beyond evdev

Remote input that bypasses evdev, such as Deskflow's Wayland/libei input on
the receiving computer, cannot be remapped: the keyboard is absent from the
device list, and its keystrokes never reach xremap. The limitation is
described for users in
[Current Features](Current_Features.md#remote-input-limitation). Supporting
it would need a different input path or a backend integration; no approach
has been investigated or selected, and a focused-window tester mode would
only display received keys rather than enable remapping.

## Flatpak

Keyloom ships as native packages only (see
[packaging/README.md](../packaging/README.md)). A Flatpak would reach
distributions without one, immutable ones among them, and could later be
published on Flathub. The direction explored so far bundles xremap in the
Flatpak and keeps the systemd user unit, its `ExecStart` running the
bundled xremap through `flatpak run --command=xremap`, so remapping still
starts at login and is restarted when it fails. The Background portal's
autostart is no substitute: COSMIC's portal backend does not implement it
yet (September 2026), nor does the wlroots one, and nothing restarts what
it starts. An approach would have to solve:

- Setup's privileged steps. Joining the `input` group and installing the
  uinput udev rule cannot happen from the sandbox, so those steps lose
  their single button that does the fix and become commands the user
  runs. "Check again" has to test access to the devices themselves, since
  the sandbox does not show the host's group membership. On Fedora Atomic
  the `input` group lives in `/usr/lib/group` and must be copied into
  `/etc/group` before `usermod` can add anyone to it.
- Paths. Flatpak points `XDG_CONFIG_HOME` at `~/.var/app/<id>/config`, so
  the unit has to be written to the host's `~/.config/systemd/user`. The
  generated configuration can stay in the app's own directory, since the
  bundled xremap runs in the same sandbox. Profiles stored through
  cosmic-config move there too, so switching from a native package does
  not carry them over.
- Permissions: `--device=all` for the input devices and `/dev/uinput`;
  `--talk-name=org.freedesktop.systemd1` and write access to
  `xdg-config/systemd`, which Flathub grants on sufficient explanation;
  read access to COSMIC's configuration for its accent color; and what
  xremap's window clients need on each desktop for application-specific
  remaps (the X11 socket, the D-Bus names of the GNOME Shell extension and
  KWin, compositor IPC sockets).
- The unit under Flatpak. `flatpak run` moves itself into a scope of its
  own, so xremap's output leaves the unit's journal; that start, stop,
  restart, and `Restart=always` still follow the process needs confirming.
  Uninstalling the Flatpak leaves the enabled unit behind, and with
  `RestartSec=5` it never reaches systemd's start limit, so the unit needs
  conditions on the app's install paths. Desktops that watch background
  apps (GNOME, KDE) see xremap as Keyloom running without a window, and
  kill it if the user denies Keyloom running in the background.
- What the sandbox hides. Detecting keyboards held by remapping reads
  `/proc`, which shows only the sandbox's own processes, and installed
  applications' desktop entries and icons are visible only with read
  access to the host's system and Flatpak export directories.
- Two ways of running xremap to keep working: the host's binary for the
  native packages and the bundled one here, where the download step and
  its updates have no place.
- The build: vendored crates for Flatpak's offline build, on the
  freedesktop runtime with its Rust extension as COSMIC's own Flatpaks
  do, and AppStream metadata, which Flathub requires and the native
  packages do not install yet.

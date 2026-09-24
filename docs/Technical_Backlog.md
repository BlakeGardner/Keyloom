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

## systemd over D-Bus instead of `systemctl`

Keyloom manages the xremap user unit by running `systemctl --user`
(`show`, `start`, `stop`, `restart`, `enable`, `daemon-reload`) and
parsing its output and error text. Each of those has a direct equivalent
on the user manager's `org.freedesktop.systemd1` session bus interface
(`LoadUnit` and the unit's properties, `StartUnit`/`StopUnit`/
`RestartUnit`, `EnableUnitFiles`, `Reload`), and zbus is already in the
dependency tree through libcosmic. Writing the unit file itself, and
backing up a user's own unit, stays file I/O: systemd has no method that
persists a unit from its text, and a transient unit cannot be enabled or
survive a logout.

Beyond removing the subprocesses, `Subscribe` plus the unit's
`PropertiesChanged` signals would let the header follow the service live
rather than as of its last query, so a crash or a stop from outside
Keyloom shows up; it would also be a prerequisite for any sandboxed
(Flatpak) build, which has no `systemctl`. An approach would have to
solve:

- Job completion: the calls return once a job is queued, so matching
  `systemctl`'s success/failure semantics means waiting for the
  `JobRemoved` signal for that job, subscribed before the call.
- A job reported `done` does not mean xremap stayed up; failures after
  spawn arrive as later state changes, which live status would surface.
- States the status model lacks today (`activating`, `deactivating`,
  `reloading`, auto-restart), and re-reading the unit after `Reloading`.
- Error reporting from D-Bus error names and messages instead of
  `systemctl`'s stderr, with `Unavailable` meaning no session bus or no
  systemd on it.
- Keeping the mapping from unit properties to status a pure function, so
  it stays unit-tested the way `parse_show` is.

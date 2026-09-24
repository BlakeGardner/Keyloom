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

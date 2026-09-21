# Technical Backlog

Engineering work that has no user-facing feature of its own: test
infrastructure, packaging, and investigations whose outcome is unknown.
Capabilities a user would notice belong in
[Upcoming Features](Upcoming_Features.md); what Keyloom does today, and the
limitations users live with, are described in
[Current Features](Current_Features.md).

Nothing here is scheduled, and an investigation listed below may conclude
that the work is not worth doing.

## End-to-end tests for the interface

The setup wizard's states are rendered to PNGs for a person or CI to look at
(`src/app/screenshots.rs`, catalogued in
[Setup_Test_Matrix.md](Setup_Test_Matrix.md)), and never asserted on. Those
storyboards could become real end-to-end tests with `iced_test`, driving the
interface and asserting on what it shows, so a regression fails the build
instead of waiting to be noticed in an image. The state matrix is worth
keeping either way, since it is the list of what needs covering.

## Input paths beyond evdev

Remote input that bypasses evdev, such as Deskflow's Wayland/libei input on
the receiving computer, cannot be remapped: the keyboard is absent from the
device list, and its keystrokes never reach xremap. The limitation is
described for users in
[Current Features](Current_Features.md#remote-input-limitation). Supporting
it would need a different input path or a backend integration; no approach
has been investigated or selected, and a focused-window tester mode would
only display received keys rather than enable remapping.

## Packaging beyond the current formats

Flatpak builds, RPMs for distributions beyond Fedora, and AppStream metadata
so software centers describe Keyloom. The current source-based Debian and
Fedora packaging is documented in
[packaging/README.md](../packaging/README.md).

For Arch Linux, `packaging/arch/PKGBUILD` builds releases from source; the
two distribution channels beyond it are blocked or deferred. Publishing to
the AUR waits for new-account registration to reopen (closed since the 2026
malware wave; announcements land on aur-general and the Arch news feed),
and needs an account, a one-time submission, and a version bump pushed
after each release. Binary packages would mean an `Arch:Extra` repository
on the Open Build Service: the PKGBUILD emitted into the source bundle
next to the `.dsc` and `.spec`, `scripts/obs.py` taught to fetch and name
`.pkg.tar.zst` files, x86_64 only (OBS offers no aarch64 Arch), and a
scratch build first to check that OBS's rolling Arch base system is
dependable enough to release from.

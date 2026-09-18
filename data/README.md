# Desktop integration

These files prepare Keyloom for future Linux packages. They are not installed
by `cargo build` or `cargo install`.

The desktop entry filename, icon name, and `StartupWMClass` use the existing
application ID, `io.github.blakegardner.Keyloom` (`src/config.rs`). Keep them
in sync if that ID ever changes. The launcher runs `keyloom` from the desktop
session's `PATH`, without a terminal or file/URL arguments.

`io.github.blakegardner.Keyloom.svg` is the single source for the application
logo. The About dialog embeds this file at compile time, and packages install
the same file as the desktop icon.

## Package layout

For a package using the `/usr` prefix:

| Source | Installed path | Mode |
| --- | --- | --- |
| `target/release/keyloom` | `/usr/bin/keyloom` | `0755` |
| `data/io.github.blakegardner.Keyloom.desktop` | `/usr/share/applications/io.github.blakegardner.Keyloom.desktop` | `0644` |
| `data/io.github.blakegardner.Keyloom.svg` | `/usr/share/icons/hicolor/scalable/apps/io.github.blakegardner.Keyloom.svg` | `0644` |

For example, from the repository root, build and stage these files in a
temporary package root (nothing is installed into the running system):

```sh
cargo build --release --locked
package_root=$(mktemp -d)
install -Dm755 target/release/keyloom "$package_root/usr/bin/keyloom"
install -Dm644 data/io.github.blakegardner.Keyloom.desktop \
    "$package_root/usr/share/applications/io.github.blakegardner.Keyloom.desktop"
install -Dm644 data/io.github.blakegardner.Keyloom.svg \
    "$package_root/usr/share/icons/hicolor/scalable/apps/io.github.blakegardner.Keyloom.svg"
```

Use the distribution's usual desktop/icon cache hooks when installing or
removing a package. Packages must also account for runtime dependencies:
Keyloom uses systemd user services and drives xremap, which its first-run
setup downloads into the user's `~/.local/bin` when the system has none (a
package may instead depend on an xremap of at least version 0.15.13, which
setup then uses as it is). The Debian package recipe that installs these
files lives in [`packaging/`](../packaging/README.md); AppStream metadata
is not provided yet.

## Validation

With `desktop-file-utils` installed, run from the repository root:

```sh
desktop-file-validate data/io.github.blakegardner.Keyloom.desktop
```

The entry follows the [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest/),
and the icon uses the shared `hicolor` theme described by the
[Icon Theme Specification](https://specifications.freedesktop.org/icon-theme/latest/).

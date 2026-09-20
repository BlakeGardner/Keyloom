//! Fallback icons for desktops without the COSMIC icon theme.
//!
//! libcosmic draws the icons of its own widgets, the header bar's
//! minimize, maximize, and close buttons among them, by name from the
//! desktop's icon themes. Its lookup searches the configured theme
//! (`Cosmic` unless COSMIC's settings say otherwise), then `Cosmic`,
//! `hicolor`, `gnome`, and `Yaru`. Pop!_OS has the first and Ubuntu the
//! last, but a desktop with neither (Debian and Fedora ship Adwaita,
//! which is never searched) finds nothing, and libcosmic draws an empty
//! icon: it embeds its icons only on platforms without icon themes.
//!
//! So Keyloom carries the icons those widgets need and, when no `Cosmic`
//! theme is installed, offers them as one. The lookup only searches
//! `icons/` under the XDG data directories, so the theme is written
//! under the user's cache directory and that directory is appended to
//! `XDG_DATA_DIRS`, last, where it can only fill gaps. Nothing is
//! written or changed on a system that has the real theme.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The theme libcosmic's lookup always searches, whatever is configured.
const THEME: &str = "Cosmic";

/// The one directory of the fallback theme, as `index.theme` names it.
const ICON_DIR: &str = "scalable/actions";

/// Hidden, so a theme picker run from Keyloom's environment never lists
/// four icons as a theme to choose.
const INDEX: &str = "\
[Icon Theme]
Name=COSMIC
Comment=The COSMIC icons Keyloom needs where the theme is not installed
Hidden=true
Directories=scalable/actions

[scalable/actions]
Context=Actions
Size=16
MinSize=8
MaxSize=512
Type=Scalable
";

/// The icons libcosmic's widgets look up by name in Keyloom's window,
/// from pop-os/cosmic-icons (CC BY-SA 4.0; see `data/icons/cosmic`).
const ICONS: [(&str, &[u8]); 4] = [
    (
        "window-close-symbolic",
        include_bytes!("../data/icons/cosmic/window-close-symbolic.svg"),
    ),
    (
        "window-maximize-symbolic",
        include_bytes!("../data/icons/cosmic/window-maximize-symbolic.svg"),
    ),
    (
        "window-minimize-symbolic",
        include_bytes!("../data/icons/cosmic/window-minimize-symbolic.svg"),
    ),
    (
        "window-restore-symbolic",
        include_bytes!("../data/icons/cosmic/window-restore-symbolic.svg"),
    ),
];

/// The value `XDG_DATA_DIRS` needs for the fallback icons to be found,
/// or `None` when the environment should be left alone: the `Cosmic`
/// theme is installed, or the fallback could not be written (reported
/// on stderr; the icons stay blank, as they were).
///
/// The caller sets the variable, before anything looks up an icon: the
/// lookup reads it once.
pub fn fallback_data_dirs() -> Option<OsString> {
    let env = Environment::read();
    let root = env.fallback_root()?;
    if env.has_cosmic_theme(&root) {
        return None;
    }
    if let Err(err) = write_theme(&root) {
        eprintln!(
            "keyloom: window button icons unavailable: cannot write {}: {err}",
            root.display()
        );
        return None;
    }
    env.data_dirs_with(&root)
}

/// The variables that decide where icon themes are searched for.
struct Environment {
    data_dirs: Option<OsString>,
    data_home: Option<OsString>,
    cache_home: Option<OsString>,
    home: Option<OsString>,
}

impl Environment {
    fn read() -> Self {
        let var = |name| std::env::var_os(name).filter(|value: &OsString| !value.is_empty());
        Self {
            data_dirs: var("XDG_DATA_DIRS"),
            data_home: var("XDG_DATA_HOME"),
            cache_home: var("XDG_CACHE_HOME"),
            home: var("HOME"),
        }
    }

    fn home(&self) -> Option<PathBuf> {
        self.home.as_deref().map(PathBuf::from)
    }

    /// The data directory holding the fallback theme:
    /// `$XDG_CACHE_HOME/keyloom/share`, usually under `~/.cache`. The
    /// icons are regenerated whenever they are missing, which is what
    /// the cache directory is for.
    fn fallback_root(&self) -> Option<PathBuf> {
        let cache =
            absolute(self.cache_home.as_deref()).or_else(|| Some(self.home()?.join(".cache")))?;
        Some(cache.join("keyloom").join("share"))
    }

    /// `XDG_DATA_DIRS` as the specification reads it: absolute entries
    /// only, and `/usr/local/share:/usr/share` when it is unset.
    fn data_dirs(&self) -> Vec<PathBuf> {
        let dirs: Vec<PathBuf> = self
            .data_dirs
            .as_deref()
            .map(|value| {
                std::env::split_paths(value)
                    .filter(|dir| dir.is_absolute())
                    .collect()
            })
            .unwrap_or_default();
        if dirs.is_empty() {
            vec!["/usr/local/share".into(), "/usr/share".into()]
        } else {
            dirs
        }
    }

    /// Whether a `Cosmic` icon theme exists anywhere the lookup
    /// searches, other than the fallback at `root` itself.
    fn has_cosmic_theme(&self, root: &Path) -> bool {
        let data_home =
            absolute(self.data_home.as_deref()).or_else(|| Some(self.home()?.join(".local/share")));
        self.data_dirs()
            .into_iter()
            .chain(data_home)
            .filter(|dir| dir != root)
            .map(|dir| dir.join("icons"))
            .chain(self.home().map(|home| home.join(".icons")))
            .any(|icons| icons.join(THEME).join("index.theme").is_file())
    }

    /// The data directories with `root` after them all, or `None` when
    /// `root` cannot be listed: a cache directory with a `:` in its name.
    fn data_dirs_with(&self, root: &Path) -> Option<OsString> {
        let dirs = self
            .data_dirs()
            .into_iter()
            .filter(|dir| dir != root)
            .chain([root.to_owned()]);
        std::env::join_paths(dirs).ok()
    }
}

/// An absolute path from a variable; the XDG specification has relative
/// ones ignored.
fn absolute(value: Option<&OsStr>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|dir| dir.is_absolute())
}

/// Write the fallback theme under `root/icons`, leaving files that are
/// already right untouched.
fn write_theme(root: &Path) -> io::Result<()> {
    let theme = root.join("icons").join(THEME);
    let icons = theme.join(ICON_DIR);
    fs::create_dir_all(&icons)?;
    write_if_changed(&theme.join("index.theme"), INDEX.as_bytes())?;
    for (name, svg) in ICONS {
        write_if_changed(&icons.join(format!("{name}.svg")), svg)?;
    }
    Ok(())
}

fn write_if_changed(path: &Path, content: &[u8]) -> io::Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    // Write-then-rename: another running Keyloom may have the old file
    // mapped, and the lookup reads `index.theme` through a mapping.
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    /// An environment rooted in a temp directory, with nothing of the
    /// machine's in it.
    fn environment(dir: &Path) -> Environment {
        Environment {
            data_dirs: Some(dir.join("system").into_os_string()),
            data_home: Some(dir.join("data").into_os_string()),
            cache_home: Some(dir.join("cache").into_os_string()),
            home: Some(dir.join("home").into_os_string()),
        }
    }

    fn install_theme(icons: &Path, name: &str) {
        let theme = icons.join(name);
        fs::create_dir_all(&theme).expect("a theme directory");
        fs::write(theme.join("index.theme"), "[Icon Theme]\n").expect("an index");
    }

    #[test]
    fn the_fallback_lives_under_the_cache_directory() {
        let dir = TempDir::new("icons-root");
        let mut env = environment(dir.path());
        assert_eq!(
            env.fallback_root(),
            Some(dir.path().join("cache/keyloom/share"))
        );

        env.cache_home = Some("relative/cache".into());
        assert_eq!(
            env.fallback_root(),
            Some(dir.path().join("home/.cache/keyloom/share"))
        );

        env.home = None;
        assert_eq!(env.fallback_root(), None);
    }

    #[test]
    fn a_desktop_with_only_other_themes_has_no_cosmic_theme() {
        let dir = TempDir::new("icons-adwaita");
        let env = environment(dir.path());
        install_theme(&dir.path().join("system/icons"), "Adwaita");
        install_theme(&dir.path().join("system/icons"), "hicolor");

        assert!(!env.has_cosmic_theme(&dir.path().join("cache/keyloom/share")));
    }

    #[test]
    fn a_cosmic_theme_is_found_wherever_the_lookup_searches() {
        for icons in ["system/icons", "data/icons", "home/.icons"] {
            let dir = TempDir::new("icons-cosmic");
            let env = environment(dir.path());
            install_theme(&dir.path().join(icons), THEME);

            assert!(
                env.has_cosmic_theme(&dir.path().join("cache/keyloom/share")),
                "{icons}"
            );
        }
    }

    #[test]
    fn the_fallback_does_not_count_as_an_installed_theme() {
        // A process started by Keyloom inherits its `XDG_DATA_DIRS`.
        let dir = TempDir::new("icons-inherited");
        let mut env = environment(dir.path());
        let root = env.fallback_root().expect("a cache directory");
        write_theme(&root).expect("a writable temp directory");
        env.data_dirs = env.data_dirs_with(&root);

        assert!(!env.has_cosmic_theme(&root));
        assert_eq!(env.data_dirs_with(&root), env.data_dirs);
    }

    #[test]
    fn the_fallback_is_searched_after_every_data_directory() {
        let root = Path::new("/cache/keyloom/share");
        let mut env = environment(Path::new("/tmp"));

        env.data_dirs = Some("/opt/share:relative:/usr/share".into());
        assert_eq!(
            env.data_dirs_with(root),
            Some("/opt/share:/usr/share:/cache/keyloom/share".into())
        );

        env.data_dirs = None;
        assert_eq!(
            env.data_dirs_with(root),
            Some("/usr/local/share:/usr/share:/cache/keyloom/share".into())
        );

        assert_eq!(env.data_dirs_with(Path::new("/odd:cache/share")), None);
    }

    #[test]
    fn the_written_theme_indexes_every_icon() {
        let dir = TempDir::new("icons-theme");
        write_theme(dir.path()).expect("a writable temp directory");
        let theme = dir.path().join("icons").join(THEME);

        let index = fs::read_to_string(theme.join("index.theme")).expect("an index");
        assert!(index.contains(&format!("Directories={ICON_DIR}\n")));
        assert!(index.contains(&format!("\n[{ICON_DIR}]\n")));
        for (name, svg) in ICONS {
            let written = fs::read(theme.join(ICON_DIR).join(format!("{name}.svg")));
            assert_eq!(written.expect("the icon"), svg, "{name}");
        }
    }

    #[test]
    fn writing_again_replaces_only_what_changed() {
        let dir = TempDir::new("icons-rewrite");
        write_theme(dir.path()).expect("a writable temp directory");
        let theme = dir.path().join("icons").join(THEME);
        let stale = theme.join(ICON_DIR).join("window-close-symbolic.svg");
        fs::write(&stale, "<svg/>").expect("a stale icon");

        write_theme(dir.path()).expect("a writable temp directory");

        assert_eq!(fs::read(&stale).expect("the icon"), ICONS[0].1);
        let leftovers: Vec<_> = fs::read_dir(theme.join(ICON_DIR))
            .expect("the icon directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn every_window_control_libcosmic_draws_is_bundled() {
        // `libcosmic/src/widget/header_bar.rs`.
        for name in [
            "window-close-symbolic",
            "window-maximize-symbolic",
            "window-minimize-symbolic",
            "window-restore-symbolic",
        ] {
            let (_, svg) = ICONS
                .iter()
                .find(|(bundled, _)| *bundled == name)
                .unwrap_or_else(|| panic!("{name} is not bundled"));
            assert!(svg.starts_with(b"<svg"), "{name}");
        }
    }
}

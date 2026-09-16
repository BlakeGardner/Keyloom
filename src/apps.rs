//! What the desktop knows about applications, for the application
//! picker: installed desktop entries (names and icons), and the windows
//! open right now, whose ids are exactly what remapping matches on.
//!
//! Open windows come from `xremap --list-windows`, which asks the
//! compositor the way the running remapper does and returns before
//! xremap selects any input device, so it never takes a keyboard away
//! from the remapper that is running.

use std::path::PathBuf;

use cosmic::desktop::{self, fde};
use tokio::process::Command;

use crate::config::APP_ID;
use crate::ui::model::AppRef;

/// An application's icon, as its desktop entry names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Icon {
    /// A themed icon name (`firefox`).
    Name(String),
    /// An image file.
    Path(PathBuf),
}

/// One application the picker can offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownApp {
    pub app: AppRef,
    pub icon: Option<Icon>,
    /// A window of it is open right now.
    pub open: bool,
    /// The title of that window, for applications nothing else names.
    pub title: Option<String>,
}

/// The picker's lists: open applications first, then the installed
/// ones, each by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    pub apps: Vec<KnownApp>,
    /// Why the open windows could not be listed, when they could not.
    pub windows_error: Option<String>,
}

/// A window the compositor reports, as xremap sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub app_id: String,
    pub title: String,
}

/// Everything the picker offers: installed applications, marked open
/// where a window of theirs is up, plus open windows of applications
/// without a desktop entry.
pub async fn catalog() -> Catalog {
    let installed = installed();
    match open_windows().await {
        Ok(windows) => Catalog {
            apps: merge(installed, &windows),
            windows_error: None,
        },
        Err(error) => Catalog {
            apps: merge(installed, &[]),
            windows_error: Some(error),
        },
    }
}

/// Installed applications, from the desktop entries the menus show.
fn installed() -> Vec<KnownApp> {
    let locales = fde::get_languages_from_env();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
    desktop::load_applications(&locales, false, desktop.as_deref())
        .map(|entry| {
            // A desktop entry's StartupWMClass names the windows of
            // applications that run through XWayland.
            let aliases = entry
                .wm_class
                .filter(|class| !class.is_empty() && class != &entry.id)
                .into_iter()
                .collect();
            KnownApp {
                app: AppRef {
                    id: entry.id,
                    name: entry.name,
                    aliases,
                },
                icon: Some(match entry.icon {
                    fde::IconSource::Name(name) => Icon::Name(name),
                    fde::IconSource::Path(path) => Icon::Path(path),
                }),
                open: false,
                title: None,
            }
        })
        .collect()
}

/// The windows open right now, as xremap reports them.
///
/// # Errors
///
/// When xremap is not installed, could not be run, or reports nothing
/// (a desktop its build cannot ask), with xremap's own explanation
/// where it gives one.
async fn open_windows() -> Result<Vec<Window>, String> {
    let output = Command::new("xremap")
        .arg("--list-windows")
        .output()
        .await
        .map_err(|err| format!("could not run xremap: {err}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !output.status.success() {
        return Err(if stderr.is_empty() {
            format!("xremap did not list windows ({})", output.status)
        } else {
            stderr
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_window_list(&stdout).ok_or_else(|| {
        if stderr.is_empty() {
            "xremap did not list any windows".to_owned()
        } else {
            stderr
        }
    })
}

/// Read the table `xremap --list-windows` prints: a header line naming
/// the `APP_CLASS`, `TITLE`, and `WIN_ID` columns, then one row per
/// window with each column padded to its widest value. `None` without
/// the header, which is what an unsupported desktop yields.
pub(crate) fn parse_window_list(stdout: &str) -> Option<Vec<Window>> {
    let mut lines = stdout.lines();
    let header = lines.next()?;
    // The header is ASCII, so byte offsets are character offsets, but
    // rows may hold any text: slice them by character.
    let title_at = header.find("TITLE")?;
    let winid_at = header.find("WIN_ID")?;
    let column = |chars: &[char], from: usize, to: usize| -> String {
        chars
            .get(from..to.min(chars.len()))
            .unwrap_or_default()
            .iter()
            .collect::<String>()
            .trim_end()
            .to_owned()
    };
    let mut windows: Vec<Window> = Vec::new();
    for line in lines {
        let chars: Vec<char> = line.chars().collect();
        let app_id = column(&chars, 0, title_at);
        if app_id.is_empty() || app_id == APP_ID {
            continue;
        }
        if windows.iter().any(|window| window.app_id == app_id) {
            continue;
        }
        windows.push(Window {
            app_id,
            title: column(&chars, title_at, winid_at),
        });
    }
    Some(windows)
}

/// Mark the installed applications that have a window open, and add
/// the open windows nothing installed accounts for.
fn merge(mut apps: Vec<KnownApp>, windows: &[Window]) -> Vec<KnownApp> {
    for window in windows {
        let id = window.app_id.as_str();
        let known = apps.iter_mut().find(|known| {
            known.app.id.eq_ignore_ascii_case(id)
                || known
                    .app
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(id))
        });
        match known {
            Some(known) => {
                // Match on what the compositor reports; the desktop
                // entry's own id stays as an alias.
                if known.app.id != id {
                    let previous = std::mem::replace(&mut known.app.id, id.to_owned());
                    known.app.aliases.retain(|alias| alias != id);
                    if !known.app.aliases.contains(&previous) {
                        known.app.aliases.push(previous);
                    }
                }
                known.open = true;
                if known.title.is_none() {
                    known.title = Some(window.title.clone());
                }
            }
            None => apps.push(KnownApp {
                app: AppRef {
                    id: id.to_owned(),
                    name: id.to_owned(),
                    aliases: Vec::new(),
                },
                icon: None,
                open: true,
                title: Some(window.title.clone()),
            }),
        }
    }
    apps.sort_by(|a, b| {
        b.open
            .cmp(&a.open)
            .then_with(|| a.app.name.to_lowercase().cmp(&b.app.name.to_lowercase()))
    });
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(id: &str, name: &str, aliases: &[&str]) -> KnownApp {
        KnownApp {
            app: AppRef {
                id: id.to_owned(),
                name: name.to_owned(),
                aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
            },
            icon: None,
            open: false,
            title: None,
        }
    }

    /// Lay rows out the way xremap's `print_table` does: every column
    /// padded to its widest value plus one space, header included.
    fn table(rows: &[[&str; 3]]) -> String {
        let mut widths = [0; 3];
        for row in std::iter::once(&["APP_CLASS", "TITLE", "WIN_ID"]).chain(rows) {
            for (width, cell) in widths.iter_mut().zip(row) {
                *width = (*width).max(cell.chars().count());
            }
        }
        let mut out = String::new();
        for row in std::iter::once(&["APP_CLASS", "TITLE", "WIN_ID"]).chain(rows) {
            for (width, cell) in widths.iter().zip(row) {
                out.push_str(cell);
                out.extend(std::iter::repeat_n(' ', width - cell.chars().count() + 1));
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn the_window_table_is_read_by_column() {
        let table = table(&[
            ["com.system76.CosmicTerm", "~ — fish", "3"],
            ["firefox", "Keyloom · GitHub — Firefox", "7"],
            ["firefox", "Second window", "9"],
            ["", "untitled", "11"],
            [APP_ID, "Keyloom", "12"],
        ]);
        let windows = parse_window_list(&table).expect("a header");
        assert_eq!(
            windows,
            vec![
                Window {
                    app_id: "com.system76.CosmicTerm".to_owned(),
                    title: "~ — fish".to_owned(),
                },
                Window {
                    app_id: "firefox".to_owned(),
                    title: "Keyloom · GitHub — Firefox".to_owned(),
                },
            ],
            "one entry per application, none for Keyloom or nameless windows"
        );
        assert_eq!(parse_window_list(""), None, "no header, no list");
        assert_eq!(parse_window_list("COSMIC is not supported.\n"), None);
    }

    #[test]
    fn open_windows_mark_installed_applications_and_add_unknown_ones() {
        let installed = vec![
            known("code", "Visual Studio Code", &["Code"]),
            known("firefox", "Firefox", &[]),
            known("com.spotify.Client", "Spotify", &[]),
        ];
        let windows = vec![
            Window {
                app_id: "steam_app_730".to_owned(),
                title: "Counter-Strike 2".to_owned(),
            },
            Window {
                app_id: "Code".to_owned(),
                title: "main.rs".to_owned(),
            },
            Window {
                app_id: "FIREFOX".to_owned(),
                title: "Tab".to_owned(),
            },
        ];
        let apps = merge(installed, &windows);
        let names: Vec<&str> = apps.iter().map(|app| app.app.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Firefox", "steam_app_730", "Visual Studio Code", "Spotify"],
            "open applications come first, each list by name"
        );
        let code = &apps[2];
        assert!(code.open);
        assert_eq!(
            code.app.id, "Code",
            "matched on what the compositor reports"
        );
        assert_eq!(code.app.aliases, vec!["code".to_owned()]);
        assert_eq!(code.title.as_deref(), Some("main.rs"));
        let firefox = &apps[0];
        assert_eq!(firefox.app.id, "FIREFOX");
        assert_eq!(firefox.app.aliases, vec!["firefox".to_owned()]);
        let game = &apps[1];
        assert!(game.open);
        assert_eq!(game.title.as_deref(), Some("Counter-Strike 2"));
        assert!(!apps[3].open);
    }
}

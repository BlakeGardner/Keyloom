//! Keyloom: a visual keyboard remapping application for Linux, built
//! with libcosmic.

mod app;
mod apps;
mod config;
mod icons;
mod install;
mod keyboard;
mod monitor;
mod service;
mod session;
mod setup;
mod systemd;
#[cfg(test)]
mod testing;
mod ui;
mod xremap;

use cosmic::theme::ThemeType;

fn main() -> cosmic::iced::Result {
    // Before anything else: the icon lookup reads `XDG_DATA_DIRS` once,
    // the first time libcosmic draws an icon.
    if let Some(data_dirs) = icons::fallback_data_dirs() {
        // SAFETY: changing the environment is unsound only while another
        // thread may be reading it, and no thread has been started yet:
        // this is the first thing `main` does, and the runtime that
        // starts them is created by `cosmic::app::run` below.
        unsafe { std::env::set_var("XDG_DATA_DIRS", data_dirs) };
    }

    let settings = cosmic::app::Settings::default()
        // Follow the desktop's light/dark preference live; the design
        // tokens in `ui::theme` adapt to the mode.
        .theme(startup_theme())
        // Fit the 100% keyboard deck like a purpose-built tool.
        .size(cosmic::iced::Size::new(1210.0, 620.0))
        .size_limits(
            cosmic::iced::core::layout::Limits::NONE
                .min_width(760.0)
                .min_height(480.0),
        );

    cosmic::app::run::<app::App>(settings, ())
}

/// The theme to start from, always one libcosmic treats as the system's.
///
/// libcosmic's default reads COSMIC's theme-mode config and, when that is
/// missing (every non-COSMIC desktop), falls back to a fixed dark theme.
/// Its XDG portal handler then applies light/dark changes only to a
/// *system* theme, so the fixed fallback would never follow GNOME or
/// KDE. Starting from the system dark theme instead lets the portal's
/// first report pick the mode and later changes keep applying.
fn startup_theme() -> cosmic::Theme {
    let preferred = cosmic::theme::system_preference();
    if matches!(preferred.theme_type, ThemeType::System { .. }) {
        preferred
    } else {
        cosmic::theme::system_dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_theme_is_a_system_theme_so_portal_changes_apply() {
        // Without this, libcosmic ignores portal light/dark reports.
        assert!(matches!(
            startup_theme().theme_type,
            ThemeType::System { .. }
        ));
    }
}

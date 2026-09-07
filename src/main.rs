//! Keyloom: a visual keyboard remapping application for Linux, built
//! with libcosmic.

mod app;
mod config;
mod keyboard;
mod monitor;
mod ui;
mod xremap;

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default()
        // Follow the system theme so COSMIC light/dark switching applies
        // live; the design tokens in `ui::theme` adapt to the mode.
        // Fit the 100% keyboard deck like a purpose-built tool.
        .size(cosmic::iced::Size::new(1210.0, 620.0))
        .size_limits(
            cosmic::iced::core::layout::Limits::NONE
                .min_width(760.0)
                .min_height(480.0),
        );

    cosmic::app::run::<app::App>(settings, ())
}

//! Keyloom: a visual keyboard remapping application for Linux, built
//! with libcosmic.

mod app;
// The layout tables in `keyboard` predate the current design; only the
// detection helpers are used (by the device monitor) until the remapping
// engine work rewires them.
#[allow(dead_code)]
mod keyboard;
mod monitor;
mod ui;

fn main() -> cosmic::iced::Result {
    // Derive the cosmic widget theme (header bar, inputs, scrollbars…)
    // from the design's background and accent tokens so native chrome
    // matches the design language.
    let bg = ui::theme::bg();
    let accent = ui::theme::accent();
    let theme = cosmic::cosmic_theme::ThemeBuilder::dark()
        .bg_color(cosmic::cosmic_theme::palette::Srgba::new(
            bg.r, bg.g, bg.b, 1.0,
        ))
        .accent(cosmic::cosmic_theme::palette::Srgb::new(
            accent.r, accent.g, accent.b,
        ))
        .build();

    let settings = cosmic::app::Settings::default()
        .theme(cosmic::theme::Theme::custom(std::sync::Arc::new(theme)))
        .size(cosmic::iced::Size::new(1280.0, 900.0));

    cosmic::app::run::<app::App>(settings, ())
}

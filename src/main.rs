//! Keyloom: an on-screen keyboard for Linux, built with libcosmic.

mod app;
mod keyboard;
mod monitor;

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size(cosmic::iced::Size::new(1000.0, 500.0));

    cosmic::app::run::<app::App>(settings, ())
}

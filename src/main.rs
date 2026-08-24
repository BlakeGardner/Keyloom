//! An on-screen keyboard for the COSMIC desktop.

mod app;
mod keyboard;
mod monitor;

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size(cosmic::iced::Size::new(1000.0, 500.0));

    cosmic::app::run::<app::App>(settings, ())
}

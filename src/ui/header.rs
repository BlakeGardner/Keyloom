//! Native header-bar content: brand, profile picker, navigation tabs,
//! and the overflow menu (`.app-header` in the export).

use std::time::Duration;

use cosmic::iced::{Alignment, Border, Length};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message, Popover, View};
use crate::service;
use crate::ui::overlays;
use crate::ui::theme::{accent, black, header_chip, muted, oklch, success, tab, white};
use crate::ui::{txt, txt_semibold};

/// Brand icon, product name, and the profile picker.
pub fn start(app: &App) -> Vec<Element<'_, Message>> {
    let icon = txt("⌨️", 17.0, oklch(0.95, 0.01, 152.0));

    let brand = txt_semibold("Keyloom", 17.0, oklch(0.95, 0.01, 152.0));

    let profile_dot = container(widget::Space::new().width(5.0).height(5.0)).class(
        ctheme::Container::custom(|_| container::Style {
            background: Some(accent().into()),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }),
    );

    let profile = widget::button::custom(
        widget::row::with_capacity(3)
            .spacing(7)
            .align_y(Alignment::Center)
            .push(profile_dot)
            .push(txt(
                app.profile_name().to_owned(),
                12.0,
                oklch(0.9, 0.01, 152.0),
            ))
            .push(txt("▾", 9.0, oklch(0.9, 0.01, 152.0))),
    )
    .class(header_chip())
    .padding([6, 10])
    .on_press(Message::TogglePopover(Popover::Profiles));

    let profile: Element<'_, Message> = if app.popover == Some(Popover::Profiles) {
        widget::popover(profile)
            .popup(overlays::profiles_popup(app))
            .position(widget::popover::Position::Bottom)
            .on_close(Message::CloseOverlays)
            .into()
    } else {
        // A quick explanation of profiles while the picker is closed.
        hint(
            profile.into(),
            "Each profile is its own set of remaps and shortcuts. \
             Click to switch or manage profiles.",
        )
    };

    vec![
        widget::row::with_capacity(3)
            .spacing(10)
            .align_y(Alignment::Center)
            .push(icon)
            .push(brand)
            .push(profile)
            .into(),
    ]
}

/// Delayed explanation hanging under a header chip.
fn hint<'a>(target: Element<'a, Message>, text: &str) -> Element<'a, Message> {
    widget::tooltip(
        target,
        txt(text.to_owned(), 11.5, oklch(0.88, 0.01, 152.0)).width(Length::Fixed(220.0)),
        widget::tooltip::Position::Bottom,
    )
    .delay(Duration::from_millis(500))
    .gap(6)
    .padding(10)
    .class(ctheme::Container::custom(|_| container::Style {
        background: Some(oklch(0.26, 0.008, 152.0).into()),
        border: Border {
            color: white(0.12),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }))
    .into()
}

/// The Keyboard / Tester / Shortcuts navigation pill.
pub fn center(app: &App) -> Vec<Element<'_, Message>> {
    let tab_button = |label: &'static str, view: View| {
        widget::button::custom(
            txt_semibold(
                label,
                12.0,
                if app.view == view {
                    oklch(0.97, 0.01, 152.0)
                } else {
                    crate::ui::theme::muted()
                },
            )
            .align_x(Alignment::Center)
            .width(Length::Fill),
        )
        .class(tab(app.view == view))
        .padding([5, 13])
        .width(Length::Fixed(92.0))
        .on_press(Message::SetView(view))
    };

    let nav = container(
        widget::row::with_capacity(3)
            .spacing(2)
            .push(tab_button("Keyboard", View::Keyboard))
            .push(tab_button("Tester", View::Tester))
            .push(tab_button("Shortcuts", View::Shortcuts)),
    )
    .padding(3)
    .class(ctheme::Container::custom(|_| container::Style {
        background: Some(black(0.3).into()),
        border: Border {
            radius: 9.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }));

    vec![nav.into()]
}

/// Amber: remapping is not carrying keys right now — paused, being
/// paused or resumed, or waiting on a change heading for the service.
const HOLDING: (f32, f32, f32) = (0.78, 0.13, 85.0);

/// The remap status chip and `⋯` overflow menu. Changes apply on
/// their own, so there is no Apply control here; the chip itself
/// pauses and starts remapping.
pub fn end(app: &App) -> Vec<Element<'_, Message>> {
    // Dot + state label. xremap and systemd are implementation details
    // the wording deliberately avoids.
    let (dot_color, label) = if let Some(target) = app.switching {
        (
            oklch(HOLDING.0, HOLDING.1, HOLDING.2),
            match target {
                service::Remapping::On => "Resuming Remapping",
                service::Remapping::Off => "Pausing Remapping",
            },
        )
    } else if app.apply_in_progress() {
        // A change is on its way to the running service.
        (oklch(HOLDING.0, HOLDING.1, HOLDING.2), "Applying Remaps")
    } else {
        match app.service {
            Some(status) => (
                match status {
                    // Traffic-light green, never the accent: the dot's
                    // colors carry meaning.
                    service::Status::Active => success(),
                    service::Status::Inactive => oklch(HOLDING.0, HOLDING.1, HOLDING.2),
                    service::Status::Failed => oklch(0.62, 0.19, 25.0),
                    // Nothing to act on: no unit, or no systemd to ask.
                    service::Status::NotFound | service::Status::Unavailable => muted(),
                },
                status.label(),
            ),
            None => (muted(), "Checking remapping…"),
        }
    };
    // Widgets are built, not cloned: each branch below makes its own row.
    let status_row = move || {
        let status_dot = container(widget::Space::new().width(7.0).height(7.0)).class(
            ctheme::Container::custom(move |_| container::Style {
                background: Some(dot_color.into()),
                border: Border {
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
        );
        widget::row::with_capacity(2)
            .spacing(6)
            .align_y(Alignment::Center)
            .push(status_dot)
            .push(txt(label, 11.0, oklch(0.85, 0.01, 152.0)))
    };

    // Pressing the chip stops or starts remapping; with nothing set up
    // it opens setup instead, and it stays passive while a restart is
    // already running or there is no systemd to ask.
    let chip = |message: Message, hint_text: &str| {
        hint(
            widget::button::custom(status_row())
                .class(header_chip())
                .padding([7, 10])
                .on_press(message)
                .into(),
            hint_text,
        )
    };
    let status: Element<'_, Message> = match app.remapping_toggle() {
        Some(target) => chip(
            Message::SetRemapping(target),
            match target {
                service::Remapping::On => "Start remapping again.",
                service::Remapping::Off => "Pause remapping.",
            },
        ),
        None if app.service == Some(service::Status::NotFound) => {
            chip(Message::MenuShowSetup, "Set up remapping.")
        }
        None => container(status_row())
            .padding([7, 10])
            .class(ctheme::Container::custom(|_| container::Style {
                background: Some(white(0.05).into()),
                border: Border {
                    color: white(0.10),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..container::Style::default()
            }))
            .into(),
    };

    let menu = widget::button::custom(
        txt("⋯", 14.0, oklch(0.85, 0.01, 152.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .class(header_chip())
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(30.0))
    .on_press(Message::TogglePopover(Popover::Menu));

    let menu: Element<'_, Message> = if app.popover == Some(Popover::Menu) {
        widget::popover(menu)
            .popup(overlays::menu_popup(app))
            .position(widget::popover::Position::Bottom)
            .on_close(Message::CloseOverlays)
            .into()
    } else {
        menu.into()
    };

    vec![
        widget::row::with_capacity(2)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(status)
            .push(menu)
            .into(),
    ]
}

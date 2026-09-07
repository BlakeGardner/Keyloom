//! Native header-bar content: brand, profile picker, navigation tabs,
//! and the overflow menu (`.app-header` in the export).

use cosmic::iced::{Alignment, Border, Length};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message, Popover, View};
use crate::ui::overlays;
use crate::ui::theme::{accent, black, header_chip, oklch, tab};
use crate::ui::{txt, txt_semibold};

/// Brand dot, product name, and the profile picker.
pub fn start(app: &App) -> Vec<Element<'_, Message>> {
    let dot = container(widget::Space::new().width(9.0).height(9.0)).class(
        ctheme::Container::custom(|_| container::Style {
            background: Some(accent().into()),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }),
    );

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
            .push(txt(app.profile_name().to_owned(), 12.0, oklch(0.9, 0.01, 152.0)))
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
        profile.into()
    };

    vec![
        widget::row::with_capacity(3)
            .spacing(10)
            .align_y(Alignment::Center)
            .push(dot)
            .push(brand)
            .push(profile)
            .into(),
    ]
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

/// The `⋯` overflow menu button.
pub fn end(app: &App) -> Vec<Element<'_, Message>> {
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

    if app.popover == Some(Popover::Menu) {
        vec![
            widget::popover(menu)
                .popup(overlays::menu_popup(app))
                .position(widget::popover::Position::Bottom)
                .on_close(Message::CloseOverlays)
                .into(),
        ]
    } else {
        vec![menu.into()]
    }
}

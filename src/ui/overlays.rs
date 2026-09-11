//! Popover panels, the confirmation toast, and modal dialogs.

use cosmic::iced::{Alignment, Border, Length, Padding};
use cosmic::widget::{self, container, mouse_area};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message, Toast, View};
use crate::keyboard;
use crate::ui::model::key_name;
use crate::ui::theme::{
    accent, accent_button, border, fg, ghost_button, menu_row, muted, oklch, quiet, scrim,
    vgradient, white,
};
use crate::ui::{eyebrow, mono, popover_panel, txt, txt_semibold};

/// A row inside a popover: name, optional subtitle, active check mark.
fn popup_row<'a>(
    name: String,
    sub: Option<Element<'a, Message>>,
    active: bool,
    message: Message,
) -> Element<'a, Message> {
    let mut labels = widget::column::with_capacity(2)
        .spacing(2)
        .push(txt_semibold(
            name,
            12.5,
            if active {
                oklch(0.97, 0.01, 152.0)
            } else {
                oklch(0.88, 0.01, 152.0)
            },
        ));
    if let Some(sub) = sub {
        labels = labels.push(sub);
    }

    let mut row = widget::row::with_capacity(3)
        .align_y(Alignment::Center)
        .push(labels)
        .push(crate::ui::hspace());
    if active {
        row = row.push(txt("✓", 11.0, accent()));
    }

    widget::button::custom(row)
        .class(menu_row(active))
        .padding([8, 10])
        .width(Length::Fill)
        .on_press(message)
        .into()
}

/// Stable widget id for the profile rename input so it can be
/// focused when rename mode is entered.
pub fn rename_input_id() -> widget::Id {
    widget::Id::new("profile-rename-input")
}

/// The profile picker popover: the user's profiles and the
/// rename / duplicate / new actions.
pub fn profiles_popup(app: &App) -> Element<'_, Message> {
    let mut column = widget::column::with_capacity(app.profiles.len() + 4).spacing(2);

    column = column.push(container(eyebrow("Your profiles")).padding(Padding {
        top: 4.0,
        right: 10.0,
        bottom: 2.0,
        left: 10.0,
    }));

    for profile in &app.profiles {
        let active = app.profile == profile.id;
        // While renaming, the active profile's row becomes an input.
        if active && let Some(name) = &app.rename {
            column = column.push(
                container(
                    widget::text_input("Profile name", name)
                        .id(rename_input_id())
                        .on_input(Message::RenameInput)
                        .on_submit(|_| Message::RenameCommit),
                )
                .padding([2, 4]),
            );
            continue;
        }
        let count = app
            .profile_maps
            .get(&profile.id)
            .map_or(0, |maps| maps.len());
        let sub = if count == 0 {
            "no mappings yet".to_owned()
        } else {
            format!("{count} mapping{}", if count == 1 { "" } else { "s" })
        };
        let row = popup_row(
            profile.name.clone(),
            Some(txt(sub, 10.5, muted()).into()),
            active,
            Message::SelectProfile(profile.id.clone()),
        );
        // Inactive profiles can be deleted; the active one is
        // protected, so at least one profile always remains.
        if active || app.view == View::Tester {
            column = column.push(row);
        } else {
            column = column.push(
                widget::row::with_capacity(2)
                    .spacing(2)
                    .align_y(Alignment::Center)
                    .push(row)
                    .push(
                        widget::button::custom(txt("✕", 11.0, oklch(0.85, 0.06, 16.0)))
                            .class(ghost_button())
                            .padding([8, 8])
                            .on_press(Message::DeleteProfile(profile.id.clone())),
                    ),
            );
        }
    }

    if app.view != View::Tester {
        column =
            column.push(container(crate::ui::keyboard_view::rule(white(0.09))).padding([6, 4]));
        let action = |label: &'static str, message: Message| {
            widget::button::custom(
                txt(label, 11.5, oklch(0.9, 0.01, 152.0))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
            .class(ghost_button())
            .padding([6, 8])
            .width(Length::Fill)
            .on_press(message)
        };
        column = column.push(
            widget::row::with_capacity(3)
                .spacing(6)
                .push(action(
                    if app.rename.is_some() {
                        "Cancel"
                    } else {
                        "Rename"
                    },
                    Message::RenameToggle,
                ))
                .push(action("Duplicate", Message::NewProfile { duplicate: true }))
                .push(
                    widget::button::custom(
                        txt_semibold("New", 11.5, oklch(0.93, 0.02, 152.0))
                            .align_x(Alignment::Center)
                            .width(Length::Fill),
                    )
                    .class(accent_button())
                    .padding([6, 8])
                    .width(Length::Fill)
                    .on_press(Message::NewProfile { duplicate: false }),
                ),
        );
    }

    popover_panel(column).width(Length::Fixed(258.0)).into()
}

/// The `Applies to` device picker popover.
pub fn devices_popup(app: &App) -> Element<'_, Message> {
    let entries = app.device_entries();
    let mut column = widget::column::with_capacity(entries.len()).spacing(2);
    for (id, name, sub) in entries {
        let active = app.device == id;
        column = column.push(popup_row(
            name,
            Some(mono(sub, 10.0, muted()).into()),
            active,
            Message::SelectDevice(id),
        ));
    }
    popover_panel(column).width(Length::Fixed(290.0)).into()
}

/// The `Size` picker popover: form factors (annotated with the detected
/// size) plus the ANSI/ISO assembly toggle.
pub fn size_popup(app: &App) -> Element<'_, Message> {
    let detected = app.detected_form();
    let mut column = widget::column::with_capacity(keyboard::FORM_FACTORS.len() + 3).spacing(2);

    for (index, form) in keyboard::FORM_FACTORS.iter().enumerate() {
        let sub = (detected == Some(index)).then(|| {
            txt(
                "Detected from your keyboard",
                10.5,
                oklch(0.75, 0.09, 152.0),
            )
            .into()
        });
        column = column.push(popup_row(
            form.name.to_owned(),
            sub,
            app.form == index,
            Message::SetForm(index),
        ));
    }

    column = column.push(container(crate::ui::keyboard_view::rule(white(0.09))).padding([6, 4]));
    for (iso, name, sub) in [
        (false, "ANSI", "US-style: bar Enter, wide left Shift"),
        (true, "ISO", "European: tall Enter, 102nd key, AltGr"),
    ] {
        column = column.push(popup_row(
            name.to_owned(),
            Some(txt(sub, 10.5, muted()).into()),
            app.iso == iso,
            Message::SetVariant(iso),
        ));
    }

    popover_panel(column).width(Length::Fixed(290.0)).into()
}

/// The `⋯` overflow menu popover.
pub fn menu_popup(app: &App) -> Element<'_, Message> {
    let mut items: Vec<(&str, Message)> = Vec::with_capacity(3);
    if app.view != View::Tester {
        items.push(("Show first-run setup", Message::MenuShowSetup));
        items.push(("Reset all mappings", Message::MenuReset));
    }
    items.push(("About Keyloom", Message::MenuAbout));

    let mut column = widget::column::with_capacity(items.len()).spacing(2);
    for (name, message) in items {
        column = column.push(popup_row(name.to_owned(), None, false, message));
    }
    popover_panel(column).width(Length::Fixed(220.0)).into()
}

/// The confirmation toast at the bottom of the shell.
pub fn toast<'a>(app: &'a App, toast: &'a Toast) -> Element<'a, Message> {
    let check = container(
        txt_semibold("✓", 9.0, oklch(0.2, 0.03, 152.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fixed(16.0))
    .height(Length::Fixed(16.0))
    .class(ctheme::Container::custom(|_| container::Style {
        background: Some(accent().into()),
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }));

    let mut undo = widget::button::custom(txt_semibold("Undo", 11.5, oklch(0.92, 0.01, 152.0)))
        .class(ghost_button())
        .padding([4, 10]);
    if app.undo.is_some() && app.view != View::Tester {
        undo = undo.on_press(Message::Undo);
    }

    let bar = container(
        widget::row::with_capacity(4)
            .spacing(12)
            .align_y(Alignment::Center)
            .push(check)
            .push(txt_semibold(
                toast.text.clone(),
                12.5,
                oklch(0.95, 0.01, 152.0),
            ))
            .push(txt(toast.sub.clone(), 11.5, muted()))
            .push(crate::ui::hspace())
            .push(undo),
    )
    .width(Length::Fill)
    .padding(Padding {
        top: 11.0,
        right: 14.0,
        bottom: 11.0,
        left: 16.0,
    })
    .class(ctheme::Container::custom(|_| container::Style {
        background: Some(oklch(0.3, 0.01, 152.0).into()),
        border: Border {
            color: white(0.13),
            width: 1.0,
            radius: 11.0.into(),
        },
        ..container::Style::default()
    }));

    container(bar)
        .width(Length::Fill)
        .padding(Padding {
            top: 16.0,
            right: 30.0,
            bottom: 16.0,
            left: 30.0,
        })
        .into()
}

/// A dimmed backdrop with a centered dialog card.
fn modal<'a>(card: Element<'a, Message>, on_backdrop: Message) -> Element<'a, Message> {
    let backdrop = mouse_area(
        container(widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .class(ctheme::Container::custom(|_| container::Style {
                background: Some(scrim().into()),
                ..container::Style::default()
            })),
    )
    .on_press(on_backdrop);

    let centered = container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .padding(16);

    cosmic::iced::widget::stack([backdrop.into(), centered.into()])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Chrome shared by the modal dialogs (`.setup-dialog`).
fn dialog_card<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(content)
        .width(Length::Fixed(540.0))
        .padding(28)
        .class(ctheme::Container::custom(|_| container::Style {
            background: Some(oklch(0.185, 0.007, 152.0).into()),
            border: Border {
                color: border(),
                width: 1.0,
                radius: 14.0.into(),
            },
            ..container::Style::default()
        }))
        .into()
}

/// The remaps dialog: every mapping in the active profile, in its own
/// window instead of crowding the main UI.
pub fn remaps_dialog(app: &App) -> Element<'_, Message> {
    let maps = app.maps();

    let header = widget::row::with_capacity(3)
        .align_y(Alignment::Center)
        .push(txt_semibold("Remaps", 24.0, fg()))
        .push(crate::ui::hspace())
        .push(
            widget::button::custom(txt("Close", 14.0, oklch(0.95, 0.01, 152.0)))
                .class(quiet(false))
                .padding([10, 15])
                .on_press(Message::CloseRemaps),
        );

    let description = txt(
        format!(
            "{} · {} mapping{} in this profile. Select a remap to edit it, or remove it here.",
            app.profile_name(),
            maps.len(),
            if maps.len() == 1 { "" } else { "s" }
        ),
        14.0,
        muted(),
    );

    let mut rows = widget::column::with_capacity(maps.len().max(1)).spacing(8);
    if maps.is_empty() {
        rows = rows.push(txt(
            "No mappings in this profile. Click a key on the keyboard to add one.",
            14.0,
            muted(),
        ));
    }
    for (code, mapping) in maps {
        let Some(cap) = crate::ui::model::key(code) else {
            continue;
        };
        let mut full_to = mapping.tap.clone().unwrap_or_else(|| key_name(code));
        if let Some(hold) = &mapping.hold {
            full_to.push_str(&format!(" · When held: {hold}"));
        }
        let scope = app.device_label(&mapping.device);

        rows = rows.push(
            widget::row::with_capacity(2)
                .spacing(8)
                .align_y(Alignment::Center)
                .push(
                    widget::button::custom(
                        widget::column::with_capacity(2)
                            .spacing(8)
                            .push(
                                widget::row::with_capacity(3)
                                    .spacing(12)
                                    .align_y(Alignment::Center)
                                    .push(mono(key_name(code), 13.0, fg()))
                                    .push(txt("→", 13.0, muted()))
                                    .push(txt_semibold(full_to, 14.0, fg())),
                            )
                            .push(txt(scope, 12.0, muted())),
                    )
                    .class(quiet(false))
                    .padding([12, 14])
                    .width(Length::Fill)
                    .on_press(Message::SelectKey(cap.code)),
                )
                .push(
                    widget::button::custom(txt("Remove", 13.0, muted()))
                        .class(quiet(false))
                        .padding([12, 14])
                        .on_press(Message::RemoveMapping(code.clone())),
                ),
        );
    }

    let card = container(
        widget::column::with_capacity(3)
            .spacing(16)
            .push(header)
            .push(description)
            .push(
                container(widget::scrollable(rows))
                    .max_height(420.0)
                    .padding(4),
            ),
    )
    .width(Length::Fixed(640.0))
    .padding(24)
    .class(ctheme::Container::custom(|_| container::Style {
        background: Some(crate::ui::theme::surface().into()),
        border: Border {
            color: border(),
            width: 1.0,
            radius: 14.0.into(),
        },
        ..container::Style::default()
    }))
    .into();

    modal(card, Message::CloseRemaps)
}

/// The "Press the key you want to use" recording dialog.
pub fn capture_dialog(app: &App) -> Element<'_, Message> {
    let subject = app.selected.map_or_else(|| "this key".to_owned(), key_name);
    let description = format!(
        "Choose the output for {subject}{}",
        match app.mode {
            crate::app::Mode::Hold => " when held.",
            crate::app::Mode::Combo => " with the modifiers you selected.",
            crate::app::Mode::Tap => ".",
        }
    );

    let listening = container(
        mono("Listening for your next key…", 14.0, fg())
            .align_x(Alignment::Center)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([22, 16])
    .class(ctheme::Container::custom(|_| container::Style {
        border: Border {
            color: border(),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }));

    let card = dialog_card(
        widget::column::with_capacity(6)
            .spacing(16)
            .push(eyebrow("Recording a key"))
            .push(txt_semibold("Press the key you want to use", 24.0, fg()))
            .push(txt(description, 15.0, muted()))
            .push(listening)
            .push(txt(
                "The key will be applied immediately. Escape cancels. To choose Escape or Tab, use the action list.",
                13.0,
                muted(),
            ))
            .push(
                widget::button::custom(
                    txt("Cancel recording", 14.0, oklch(0.95, 0.01, 152.0))
                        .align_x(Alignment::Center)
                        .width(Length::Fill),
                )
                .class(quiet(false))
                .padding([10, 15])
                .width(Length::Fill)
                .on_press(Message::SetCapture(false)),
            )
            .into(),
    );

    modal(card, Message::SetCapture(false))
}

/// The delete-profile confirmation dialog.
pub fn delete_profile_dialog(app: &App) -> Element<'_, Message> {
    let profile = app
        .confirm_delete
        .as_ref()
        .and_then(|id| app.profiles.iter().find(|profile| &profile.id == id));
    let name = profile.map_or("this profile", |profile| profile.name.as_str());
    let count = profile
        .and_then(|profile| app.profile_maps.get(&profile.id))
        .map_or(0, |maps| maps.len());
    let body = match count {
        0 => "It has no mappings. You can undo right after deleting.".to_owned(),
        1 => "Its 1 mapping is deleted with it. You can undo right after.".to_owned(),
        n => format!("Its {n} mappings are deleted with it. You can undo right after."),
    };

    let buttons = widget::row::with_capacity(3)
        .spacing(10)
        .push(crate::ui::hspace())
        .push(
            widget::button::custom(txt_semibold("Cancel", 12.5, oklch(0.85, 0.01, 152.0)))
                .class(ghost_button())
                .padding([9, 16])
                .on_press(Message::DeleteCancel),
        )
        .push(
            widget::button::custom(txt_semibold(
                "Delete profile",
                12.5,
                oklch(0.85, 0.06, 16.0),
            ))
            .class(quiet(false))
            .padding([9, 18])
            .on_press(Message::DeleteConfirm),
        );

    let card = dialog_card(
        widget::column::with_capacity(4)
            .spacing(16)
            .push(eyebrow("Delete profile"))
            .push(txt_semibold(format!("Delete {name}?"), 24.0, fg()))
            .push(txt(body, 15.0, muted()))
            .push(buttons)
            .into(),
    );

    modal(card, Message::DeleteCancel)
}

/// Application identity and credits, using the first-run dialog chrome.
pub fn about_dialog() -> Element<'static, Message> {
    use cosmic::iced::widget::svg;

    let logo = svg(svg::Handle::from_memory(
        include_bytes!("../../assets/keyloom_logo.svg").as_slice(),
    ))
    .width(Length::Fixed(144.0))
    .height(Length::Fixed(144.0));

    let card = dialog_card(
        widget::column::with_capacity(6)
            .spacing(16)
            .align_x(Alignment::Center)
            .push(logo)
            .push(txt_semibold("About Keyloom", 26.0, fg()))
            .push(txt(
                concat!("Version ", env!("CARGO_PKG_VERSION")),
                13.0,
                muted(),
            ))
            .push(
                txt(
                    "Make your keyboard your own. Remap keys, create shortcuts, and switch profiles with a visual keyboard editor for Linux.",
                    14.0,
                    muted(),
                )
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .push(
                txt("Built with Rust and libcosmic. Powered by xremap.", 12.0, muted())
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .push(
                widget::button::custom(txt_semibold("Close", 12.5, oklch(0.96, 0.02, 152.0)))
                    .class(accent_button())
                    .padding([9, 24])
                    .on_press(Message::CloseAbout),
            )
            .into(),
    );

    modal(card, Message::CloseAbout)
}

/// The three-step first-run walkthrough.
pub fn onboarding(app: &App) -> Element<'_, Message> {
    let step = app.onb_step.min(2);
    let (cap, title, body, cta, foot) = crate::ui::model::ONBOARDING[step];

    let cap = container(
        txt_semibold(
            cap,
            if step == 1 { 22.0 } else { 34.0 },
            oklch(0.98, 0.02, 152.0),
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fixed(96.0))
    .height(Length::Fixed(96.0))
    .class(ctheme::Container::custom(|_| container::Style {
        background: Some(vgradient(oklch(0.4, 0.08, 152.0), oklch(0.33, 0.07, 152.0))),
        border: Border {
            color: accent(),
            width: 1.0,
            radius: 11.0.into(),
        },
        ..container::Style::default()
    }));

    let mut dots = widget::row::with_capacity(3)
        .spacing(10)
        .align_y(Alignment::Center);
    for index in 0..3 {
        let active = index == step;
        dots = dots.push(
            container(widget::Space::new())
                .width(Length::Fixed(if active { 18.0 } else { 6.0 }))
                .height(Length::Fixed(6.0))
                .class(ctheme::Container::custom(move |_| container::Style {
                    background: Some(if active {
                        accent().into()
                    } else {
                        white(0.16).into()
                    }),
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })),
        );
    }

    let buttons = widget::row::with_capacity(2)
        .spacing(10)
        .push(
            widget::button::custom(txt_semibold("Skip setup", 12.5, oklch(0.85, 0.01, 152.0)))
                .class(ghost_button())
                .padding([9, 16])
                .on_press(Message::SkipOnboarding),
        )
        .push(
            widget::button::custom(txt_semibold(cta, 12.5, oklch(0.96, 0.02, 152.0)))
                .class(accent_button())
                .padding([9, 18])
                .on_press(Message::NextOnboarding),
        );

    let card = dialog_card(
        widget::column::with_capacity(6)
            .spacing(20)
            .align_x(Alignment::Center)
            .padding(Padding {
                top: 12.0,
                right: 32.0,
                bottom: 8.0,
                left: 32.0,
            })
            .push(cap)
            .push(txt_semibold(title, 26.0, oklch(0.96, 0.01, 152.0)))
            .push(
                txt(body, 14.0, muted())
                    .align_x(Alignment::Center)
                    .width(Length::Fixed(420.0)),
            )
            .push(dots)
            .push(buttons)
            .push(txt(foot, 11.0, muted()))
            .into(),
    );

    modal(card, Message::SkipOnboarding)
}

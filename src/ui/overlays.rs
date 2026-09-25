//! Popover panels, the confirmation toast, and modal dialogs.

use std::path::Path;

use cosmic::iced::core::text::Wrapping;
use cosmic::iced::widget::{rich_text, span};
use cosmic::iced::{Alignment, Border, Color, Font, Length, Padding};
use cosmic::widget::{self, container, icon, mouse_area};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message, Picker, PickerTarget, Setup, SetupPage, Toast, View};
use crate::apps;
use crate::install;
use crate::keyboard;
use crate::service;
use crate::session::Desktop;
use crate::setup::{
    ActionError, AppMatching, Facts, GroupCheck, INPUT_GROUP, RULES_PATH, Step, UINPUT,
    UinputCheck, UnitCheck, XREMAP_GNOME_EXTENSION_URL, XREMAP_NO_SUDO_URL, XREMAP_URL,
    XremapAction, XremapCheck, uinput_commands,
};
use crate::ui::theme::{
    accent, accent_button, border, fg, ghost_button, menu_row, muted, oklch, quiet, scrim, tint,
    white,
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

/// Shared compact destructive action for profile and remap rows.
fn remove_button(message: Message) -> Element<'static, Message> {
    widget::button::custom(txt("✕", 11.0, oklch(0.85, 0.06, 16.0)))
        .class(ghost_button())
        .padding([8, 8])
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
        let layers = app.profile_layers.get(&profile.id).map_or(0, Vec::len);
        let apps = app.profile_apps.get(&profile.id).map_or(0, Vec::len);
        let mut sub = if count == 0 {
            "no mappings yet".to_owned()
        } else {
            format!("{count} mapping{}", if count == 1 { "" } else { "s" })
        };
        if layers > 0 {
            sub.push_str(&format!(
                " · {layers} layer{}",
                if layers == 1 { "" } else { "s" }
            ));
        }
        if apps > 0 {
            sub.push_str(&format!(
                " · {apps} app{}",
                if apps == 1 { "" } else { "s" }
            ));
        }
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
                    .push(remove_button(Message::DeleteProfile(profile.id.clone()))),
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
                        txt_semibold("New", 11.5, tint(0.93, 0.02))
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
    let detected_iso = app.detected_iso();
    let detected_note = if app.device == "all" {
        "Detected from connected keyboards"
    } else {
        "Detected from this keyboard"
    };
    let mut column = widget::column::with_capacity(keyboard::FORM_FACTORS.len() + 3).spacing(2);

    for (index, form) in keyboard::FORM_FACTORS.iter().enumerate() {
        let sub =
            (detected == Some(index)).then(|| txt(detected_note, 10.5, tint(0.75, 0.09)).into());
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
        let mut details = widget::column::with_capacity(2)
            .spacing(2)
            .push(txt(sub, 10.5, muted()));
        if detected_iso == Some(iso) {
            details = details.push(txt(detected_note, 10.5, tint(0.75, 0.09)));
        }
        column = column.push(popup_row(
            name.to_owned(),
            Some(details.into()),
            app.iso == iso,
            Message::SetVariant(iso),
        ));
    }

    popover_panel(column).width(Length::Fixed(290.0)).into()
}

/// The `⋯` overflow menu popover.
pub fn menu_popup(app: &App) -> Element<'_, Message> {
    let mut items: Vec<(&str, Message)> = Vec::with_capacity(3);
    items.push(("Set up remapping", Message::MenuShowSetup));
    if app.view != View::Tester {
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
        txt_semibold("✓", 9.0, tint(0.2, 0.03))
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

    let mut content = widget::row::with_capacity(5)
        .spacing(12)
        .align_y(Alignment::Center)
        .push(check)
        .push(txt_semibold(
            toast.text.clone(),
            12.5,
            oklch(0.95, 0.01, 152.0),
        ))
        .push(txt(toast.sub.clone(), 11.5, muted()))
        .push(crate::ui::hspace());
    if app.undo.is_some() && app.view != View::Tester {
        content = content.push(
            widget::button::custom(txt_semibold("Undo", 11.5, oklch(0.92, 0.01, 152.0)))
                .class(ghost_button())
                .padding([4, 10])
                .on_press(Message::Undo),
        );
    }

    let bar = container(content)
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

/// A dimmed backdrop with a centered dialog card. Pressing the backdrop
/// sends `on_backdrop`; with `None` the scrim still swallows the press
/// so it cannot reach the page beneath, but the dialog stays open.
fn modal<'a, Renderer: cosmic::iced::core::Renderer + 'a>(
    card: cosmic::iced::Element<'a, Message, cosmic::Theme, Renderer>,
    on_backdrop: Option<Message>,
) -> cosmic::iced::Element<'a, Message, cosmic::Theme, Renderer> {
    let backdrop = mouse_area(
        container(widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .class(ctheme::Container::custom(|_| container::Style {
                background: Some(scrim().into()),
                ..container::Style::default()
            })),
    );
    let backdrop = match on_backdrop {
        Some(message) => backdrop.on_press(message),
        None => backdrop,
    };

    // Capture presses on the card's text and padding before they reach the scrim.
    // Keep the full-window centering container transparent to backdrop clicks.
    let centered = container(cosmic::iced::widget::opaque(card))
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
        .padding(CARD_PADDING)
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

    // Keys no deck draws (media, brightness, Mission Control) are
    // reached by pressing them.
    let choose = widget::row::with_capacity(2)
        .spacing(12)
        .align_y(Alignment::Center)
        .push(
            widget::button::custom(txt(
                "Remap a key that isn't shown…",
                13.0,
                oklch(0.95, 0.01, 152.0),
            ))
            .class(quiet(false))
            .padding([8, 12])
            .on_press(Message::ChooseKey),
        )
        .push(txt(
            "Press it on your keyboard: media, brightness, and Apple function keys aren't drawn above.",
            12.0,
            muted(),
        ));

    let mut rows = widget::column::with_capacity(maps.len().max(1)).spacing(8);
    if maps.is_empty() {
        rows = rows.push(txt(
            "No mappings in this profile. Click a key on the keyboard to add one.",
            14.0,
            muted(),
        ));
    }
    for (index, (code, mapping)) in maps.iter().enumerate() {
        if crate::ui::model::key(code).is_none() {
            continue;
        }
        let mut full_to = if mapping.normal {
            format!("{} · normal here", app.key_name(code))
        } else {
            mapping.tap.clone().unwrap_or_else(|| app.key_name(code))
        };
        if let Some(hold) = &mapping.hold {
            full_to.push_str(&format!(" · When held: {hold}"));
        }
        let scope = app.scope_label(mapping);

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
                                    .push(mono(app.key_name(code), 13.0, fg()))
                                    .push(txt("→", 13.0, muted()))
                                    .push(txt_semibold(full_to, 14.0, fg())),
                            )
                            .push(txt(scope, 12.0, muted())),
                    )
                    .class(quiet(false))
                    .padding([12, 14])
                    .width(Length::Fill)
                    .on_press(Message::EditMapping(index)),
                )
                .push(remove_button(Message::RemoveMapping(index))),
        );
    }

    let card = container(
        widget::column::with_capacity(4)
            .spacing(16)
            .push(header)
            .push(description)
            .push(
                // Reserve a scrollbar gutter so it cannot cover the remove buttons
                // or intercept their hover and click events.
                container(widget::scrollable(rows).spacing(8))
                    .max_height(360.0)
                    .padding(4),
            )
            .push(choose),
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

    modal(card, Some(Message::CloseRemaps))
}

/// The "Press the key you want to use" recording dialog.
pub fn capture_dialog(app: &App) -> Element<'_, Message> {
    let subject = app
        .selected
        .map_or_else(|| "this key".to_owned(), |code| app.key_name(code));
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

    modal(card, Some(Message::SetCapture(false)))
}

/// The "Press the key you want to remap" dialog, for keys no deck
/// draws.
pub fn choose_key_dialog() -> Element<'static, Message> {
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
            .push(eyebrow("Choosing a key"))
            .push(txt_semibold("Press the key you want to remap", 24.0, fg()))
            .push(txt(
                "Any key works, including the ones the keyboard above doesn't draw: media and brightness keys, and Mission Control or Launchpad on Apple keyboards. The editor opens for it.",
                15.0,
                muted(),
            ))
            .push(listening)
            .push(txt(
                "Modifier keys are ignored while listening. Escape cancels.",
                13.0,
                muted(),
            ))
            .push(
                widget::button::custom(
                    txt("Cancel", 14.0, oklch(0.95, 0.01, 152.0))
                        .align_x(Alignment::Center)
                        .width(Length::Fill),
                )
                .class(quiet(false))
                .padding([10, 15])
                .width(Length::Fill)
                .on_press(Message::CancelChooseKey),
            )
            .into(),
    );

    modal(card, Some(Message::CancelChooseKey))
}

/// Confirm removal while keeping the remaps list open underneath.
pub fn remove_mapping_dialog(app: &App) -> Element<'_, Message> {
    let entry = app
        .confirm_remove_mapping
        .and_then(|index| app.maps().get(index));
    let name = entry.map_or_else(|| "this key".to_owned(), |(code, _)| app.key_name(code));
    let scope = entry
        .map(|(_, mapping)| mapping)
        .filter(|mapping| !mapping.is_general())
        .map_or_else(String::new, |mapping| {
            format!(" for {}", app.scope_label(mapping))
        });
    let buttons = widget::row::with_capacity(3)
        .spacing(10)
        .push(crate::ui::hspace())
        .push(
            widget::button::custom(txt_semibold("Cancel", 12.5, oklch(0.85, 0.01, 152.0)))
                .class(ghost_button())
                .padding([9, 16])
                .on_press(Message::RemoveMappingCancel),
        )
        .push(
            widget::button::custom(txt_semibold("Remove remap", 12.5, oklch(0.85, 0.06, 16.0)))
                .class(quiet(false))
                .padding([9, 18])
                .on_press(Message::RemoveMappingConfirm),
        );
    let card = dialog_card(
        widget::column::with_capacity(4)
            .spacing(16)
            .push(eyebrow("Remove remap"))
            .push(txt_semibold(format!("Remove remap for {name}?"), 24.0, fg()))
            .push(txt(
                format!(
                    "This removes the remap{scope} from {} and restores the key's behavior there. You can undo right after removing.",
                    app.profile_name()
                ),
                15.0,
                muted(),
            ))
            .push(buttons)
            .into(),
    );
    modal(card, Some(Message::RemoveMappingCancel))
}

/// The delete-layer confirmation dialog.
pub fn delete_layer_dialog(app: &App) -> Element<'_, Message> {
    let layer = app
        .confirm_delete_layer
        .as_ref()
        .and_then(|id| app.layers().iter().find(|layer| &layer.id == id));
    let name = layer.map_or("this layer", |layer| layer.name.as_str());
    let body = layer.map_or_else(
        || "You can undo right after deleting.".to_owned(),
        |layer| {
            let key = app.key_name(&layer.trigger);
            let jobs = match layer.keys.len() {
                0 => String::new(),
                1 => "Its 1 key goes back to normal, and holding ".to_owned(),
                n => format!("Its {n} keys go back to normal, and holding "),
            };
            if jobs.is_empty() {
                format!("Holding {key} will do nothing special any more. You can undo right after deleting.")
            } else {
                format!("{jobs}{key} will do nothing special any more. You can undo right after deleting.")
            }
        },
    );

    let buttons = widget::row::with_capacity(3)
        .spacing(10)
        .push(crate::ui::hspace())
        .push(
            widget::button::custom(txt_semibold("Cancel", 12.5, oklch(0.85, 0.01, 152.0)))
                .class(ghost_button())
                .padding([9, 16])
                .on_press(Message::DeleteLayerCancel),
        )
        .push(
            widget::button::custom(txt_semibold("Delete layer", 12.5, oklch(0.85, 0.06, 16.0)))
                .class(quiet(false))
                .padding([9, 18])
                .on_press(Message::DeleteLayerConfirm),
        );

    let card = dialog_card(
        widget::column::with_capacity(4)
            .spacing(16)
            .push(eyebrow("Delete layer"))
            .push(txt_semibold(format!("Delete {name}?"), 24.0, fg()))
            .push(txt(body, 15.0, muted()))
            .push(buttons)
            .into(),
    );

    modal(card, Some(Message::DeleteLayerCancel))
}

/// The delete-application-scope confirmation dialog.
pub fn delete_app_dialog(app: &App) -> Element<'_, Message> {
    let scope = app
        .confirm_delete_app
        .as_ref()
        .and_then(|id| app.app_scopes().iter().find(|scope| &scope.id == id));
    let name = scope.map_or("this application", |scope| scope.name.as_str());
    let body = scope.map_or_else(
        || "You can undo right after removing.".to_owned(),
        |scope| {
            let keys = app
                .maps()
                .iter()
                .filter(|(_, mapping)| mapping.app == scope.id)
                .count();
            let rules: usize = app
                .groups()
                .iter()
                .filter(|group| group.scope == scope.id)
                .map(|group| group.rules.len())
                .sum();
            let mut body = match keys {
                0 => "No key differs there yet.".to_owned(),
                1 => "Its 1 key works like everywhere else again.".to_owned(),
                n => format!("Its {n} keys work like everywhere else again."),
            };
            match rules {
                0 => {}
                1 => body.push_str(" Its shortcut goes with it."),
                n => body.push_str(&format!(" Its {n} shortcuts go with it.")),
            }
            body.push_str(" You can undo right after removing.");
            body
        },
    );

    let buttons = widget::row::with_capacity(3)
        .spacing(10)
        .push(crate::ui::hspace())
        .push(
            widget::button::custom(txt_semibold("Cancel", 12.5, oklch(0.85, 0.01, 152.0)))
                .class(ghost_button())
                .padding([9, 16])
                .on_press(Message::DeleteAppScopeCancel),
        )
        .push(
            widget::button::custom(txt_semibold(
                "Remove application",
                12.5,
                oklch(0.85, 0.06, 16.0),
            ))
            .class(quiet(false))
            .padding([9, 18])
            .on_press(Message::DeleteAppScopeConfirm),
        );

    let card = dialog_card(
        widget::column::with_capacity(4)
            .spacing(16)
            .push(eyebrow("Remove application"))
            .push(txt_semibold(format!("Remove {name}?"), 24.0, fg()))
            .push(txt(body, 15.0, muted()))
            .push(buttons)
            .into(),
    );

    modal(card, Some(Message::DeleteAppScopeCancel))
}

/// The scope chooser of one shortcut group: every application, each
/// application scope of the profile, or a new one.
pub fn group_scope_popup(app: &App, index: usize) -> Element<'_, Message> {
    let current = app
        .groups()
        .get(index)
        .map_or("", |group| group.scope.as_str());
    let mut column = widget::column::with_capacity(app.app_scopes().len() + 3).spacing(2);
    column = column.push(popup_row(
        "All applications".to_owned(),
        None,
        current.is_empty(),
        Message::SetGroupScope {
            group: index,
            scope: String::new(),
        },
    ));
    for scope in app.app_scopes() {
        column = column.push(popup_row(
            scope.name.clone(),
            Some(txt(scope.members(), 10.5, muted()).into()),
            current == scope.id,
            Message::SetGroupScope {
                group: index,
                scope: scope.id.clone(),
            },
        ));
    }
    column = column.push(container(crate::ui::keyboard_view::rule(white(0.09))).padding([6, 4]));
    column = column.push(popup_row(
        "New application…".to_owned(),
        None,
        false,
        Message::GroupAppScope(index),
    ));
    popover_panel(column).width(Length::Fixed(258.0)).into()
}

/// The side chooser of one modifier in the edited shortcut's input
/// chord: either key of the pair, or one side alone.
pub fn modifier_side_popup(app: &App, index: usize) -> Element<'_, Message> {
    use crate::ui::model::{MODS, ModifierSide, parse_modifier};
    let current = app
        .edit_rule
        .and_then(|edit| app.groups().get(edit.group).zip(edit.rule))
        .and_then(|(group, rule)| group.rules.get(rule))
        .and_then(|rule| rule.from.mods.get(index))
        .and_then(|name| parse_modifier(name));
    let Some((family, side)) = current else {
        return popover_panel(txt("No modifier here.", 12.0, muted()))
            .width(Length::Fixed(220.0))
            .into();
    };
    let key = MODS[family];
    let mut column = widget::column::with_capacity(3).spacing(2);
    for (choice, name, sub) in [
        (
            ModifierSide::Either,
            format!("Either {key} key"),
            "matches the left and the right one",
        ),
        (
            ModifierSide::Left,
            format!("Left {key} only"),
            "the right one leaves this shortcut alone",
        ),
        (
            ModifierSide::Right,
            format!("Right {key} only"),
            "the left one leaves this shortcut alone",
        ),
    ] {
        column = column.push(popup_row(
            name,
            Some(txt(sub, 10.5, muted()).into()),
            side == choice,
            Message::SetModifierSide {
                index,
                side: choice,
            },
        ));
    }
    popover_panel(column).width(Length::Fixed(258.0)).into()
}

/// The application picker: the applications open right now, named
/// exactly as remapping sees them, then the installed ones, and a way
/// to type a name for anything else.
#[allow(clippy::too_many_lines)]
pub fn picker_dialog<'a>(app: &'a App, picker: &'a Picker) -> Element<'a, Message> {
    let (title, confirm) = match &picker.target {
        PickerTarget::Scope(_) => ("Change applications", "Save"),
        PickerTarget::NewScope | PickerTarget::Group(_) => ("Add an application", "Add"),
    };
    let editing = match &picker.target {
        PickerTarget::Scope(id) => Some(id.as_str()),
        PickerTarget::NewScope | PickerTarget::Group(_) => None,
    };
    let query = picker.query.trim().to_lowercase();

    let header = widget::row::with_capacity(3)
        .align_y(Alignment::Center)
        .push(txt_semibold(title, 24.0, fg()))
        .push(crate::ui::hspace())
        .push(
            widget::button::custom(txt("Close", 14.0, oklch(0.95, 0.01, 152.0)))
                .class(quiet(false))
                .padding([10, 15])
                .on_press(Message::PickerCancel),
        );

    let section = |label: &str, note: &str| -> Element<'a, Message> {
        let mut row = widget::row::with_capacity(2)
            .spacing(10)
            .align_y(Alignment::Center)
            .push(eyebrow(label));
        if !note.is_empty() {
            row = row.push(txt(note.to_owned(), 11.0, muted()));
        }
        container(row)
            .padding(Padding {
                top: 8.0,
                right: 10.0,
                bottom: 2.0,
                left: 10.0,
            })
            .into()
    };

    let mut list = widget::column::with_capacity(12).spacing(2);
    match &picker.catalog {
        None => {
            list = list
                .push(container(txt("Looking for applications…", 13.0, muted())).padding([12, 10]));
        }
        Some(catalog) => {
            let matches = |known: &&apps::KnownApp| {
                query.is_empty()
                    || known.app.name.to_lowercase().contains(&query)
                    || known.app.id.to_lowercase().contains(&query)
            };
            let open: Vec<&apps::KnownApp> = catalog
                .apps
                .iter()
                .filter(|known| known.open)
                .filter(matches)
                .collect();
            let installed: Vec<&apps::KnownApp> = catalog
                .apps
                .iter()
                .filter(|known| !known.open)
                .filter(matches)
                .collect();
            if !open.is_empty() {
                list = list.push(section("Open now", "named exactly as remapping sees them"));
                for known in open {
                    list = list.push(app_row(app, picker, known, editing));
                }
            } else if let Some(error) = &catalog.windows_error {
                list = list.push(
                    container(
                        txt(
                            format!("Keyloom could not ask which applications are open: {error}"),
                            12.0,
                            muted(),
                        )
                        .width(Length::Fill),
                    )
                    .padding([6, 10]),
                );
            }
            if !installed.is_empty() {
                list = list.push(section("Installed", ""));
                for known in installed {
                    list = list.push(app_row(app, picker, known, editing));
                }
            }
            if catalog.apps.is_empty()
                || (!query.is_empty() && list_is_empty(&catalog.apps, &query))
            {
                list = list.push(
                    container(txt("No application matches.", 13.0, muted())).padding([12, 10]),
                );
            }
        }
    }

    // Anything not listed: open it and it shows under "Open now", or
    // name it by the id its windows report.
    let custom = widget::row::with_capacity(2)
        .spacing(8)
        .align_y(Alignment::Center)
        .push(
            widget::text_input(
                "Not listed? Type the id its windows report…",
                &picker.custom,
            )
            .on_input(Message::PickerCustom)
            .on_submit(|_| Message::PickerAddCustom),
        )
        .push(
            widget::button::custom(txt_semibold("Add", 12.5, oklch(0.85, 0.01, 152.0)))
                .class(ghost_button())
                .padding([9, 16])
                .on_press(Message::PickerAddCustom),
        );

    let chosen: Vec<&str> = picker.chosen.iter().map(|app| app.name.as_str()).collect();
    let chosen_line = if chosen.is_empty() {
        txt("Choose one or more applications.", 12.5, muted())
    } else {
        txt(format!("Chosen: {}", chosen.join(", ")), 12.5, fg())
    };
    let mut done = widget::button::custom(txt_semibold(confirm, 12.5, tint(0.96, 0.02)))
        .class(accent_button())
        .padding([9, 18]);
    if !picker.chosen.is_empty() {
        done = done.on_press(Message::PickerConfirm);
    }
    let footer = widget::row::with_capacity(4)
        .spacing(10)
        .align_y(Alignment::Center)
        .push(chosen_line.width(Length::Fill))
        .push(
            widget::button::custom(txt_semibold("Cancel", 12.5, oklch(0.85, 0.01, 152.0)))
                .class(ghost_button())
                .padding([9, 16])
                .on_press(Message::PickerCancel),
        )
        .push(done);

    // Kept short enough to fit the default window together with its
    // footer.
    let card = container(
        widget::column::with_capacity(5)
            .spacing(12)
            .push(header)
            .push(
                widget::text_input("Search applications…", &picker.query)
                    .on_input(Message::PickerQuery),
            )
            .push(
                container(widget::scrollable(list).spacing(8))
                    .max_height(230.0)
                    .padding(4),
            )
            .push(custom)
            .push(footer),
    )
    .width(Length::Fixed(600.0))
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

    modal(card, Some(Message::PickerCancel))
}

/// Whether no application matches the picker's search.
fn list_is_empty(apps: &[apps::KnownApp], query: &str) -> bool {
    !apps.iter().any(|known| {
        known.app.name.to_lowercase().contains(query) || known.app.id.to_lowercase().contains(query)
    })
}

/// One application in the picker: icon, name, where it stands, and a
/// check mark once chosen.
fn app_row<'a>(
    app: &'a App,
    picker: &Picker,
    known: &'a apps::KnownApp,
    editing: Option<&str>,
) -> Element<'a, Message> {
    let chosen = picker.has(&known.app.id);
    let elsewhere = app
        .scope_of_app(&known.app.id)
        .filter(|scope| Some(scope.id.as_str()) != editing)
        .map(|scope| scope.name.clone());
    let icon: Element<'a, Message> = match &known.icon {
        Some(apps::Icon::Name(name)) => icon::from_name(name.as_str()).size(20).icon().into(),
        Some(apps::Icon::Path(path)) => icon::icon(icon::from_path(path.clone())).size(20).into(),
        None => widget::Space::new().width(20.0).height(20.0).into(),
    };
    let sub = elsewhere.as_ref().map_or_else(
        || {
            known
                .title
                .clone()
                .filter(|_| known.open && known.app.name != known.app.id)
                .unwrap_or_else(|| known.app.id.clone())
        },
        |scope| format!("already in {scope}"),
    );
    let labels = widget::column::with_capacity(2)
        .spacing(2)
        .push(txt_semibold(
            known.app.name.clone(),
            12.5,
            if elsewhere.is_some() {
                muted()
            } else {
                oklch(0.92, 0.01, 152.0)
            },
        ))
        .push(txt(sub, 10.5, muted()));
    let mut row = widget::row::with_capacity(4)
        .spacing(10)
        .align_y(Alignment::Center)
        .push(icon)
        .push(labels)
        .push(crate::ui::hspace());
    if chosen {
        row = row.push(txt("✓", 11.0, accent()));
    }
    widget::button::custom(row)
        .class(menu_row(chosen))
        .padding([8, 10])
        .width(Length::Fill)
        .on_press(Message::PickerToggle(known.app.clone()))
        .into()
}

/// Confirm clearing all mappings and layers in the active profile.
pub fn reset_mappings_dialog(app: &App) -> Element<'_, Message> {
    let buttons = widget::row::with_capacity(3)
        .spacing(10)
        .push(crate::ui::hspace())
        .push(
            widget::button::custom(txt_semibold("Cancel", 12.5, oklch(0.85, 0.01, 152.0)))
                .class(ghost_button())
                .padding([9, 16])
                .on_press(Message::ResetMappingsCancel),
        )
        .push(
            widget::button::custom(txt_semibold(
                "Reset all mappings",
                12.5,
                oklch(0.85, 0.06, 16.0),
            ))
            .class(quiet(false))
            .padding([9, 18])
            .on_press(Message::ResetMappingsConfirm),
        );
    let card = dialog_card(
        widget::column::with_capacity(4)
            .spacing(16)
            .push(eyebrow("Reset mappings"))
            .push(txt_semibold("Reset all mappings?", 24.0, fg()))
            .push(txt(
                format!(
                    "This removes all mappings, layers, applications, and shortcuts from {} and restores the keys' original behavior.",
                    app.profile_name()
                ),
                15.0,
                muted(),
            ))
            .push(buttons)
            .into(),
    );
    modal(card, Some(Message::ResetMappingsCancel))
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
    let layers = profile
        .and_then(|profile| app.profile_layers.get(&profile.id))
        .map_or(0, Vec::len);
    let mut body = match count {
        0 => "It has no mappings.".to_owned(),
        1 => "Its 1 mapping is deleted with it.".to_owned(),
        n => format!("Its {n} mappings are deleted with it."),
    };
    match layers {
        0 => {}
        1 => body.push_str(" Its layer goes with it."),
        n => body.push_str(&format!(" Its {n} layers go with it.")),
    }
    body.push_str(" You can undo right after deleting.");

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

    modal(card, Some(Message::DeleteCancel))
}

/// Application identity, license, and credits, using the first-run dialog chrome.
pub fn about_dialog() -> Element<'static, Message> {
    use cosmic::iced::widget::svg;

    let logo = svg(svg::Handle::from_memory(
        include_bytes!("../../data/io.github.blakegardner.Keyloom.svg").as_slice(),
    ))
    .width(Length::Fixed(144.0))
    .height(Length::Fixed(144.0));

    let card = dialog_card(
        widget::column::with_capacity(8)
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
                // Centered through the container, not the text: rich text
                // places its link spans as if it were left-aligned.
                container(
                    rich_text([
                        span("Built with Rust and libcosmic. Powered by "),
                        span("xremap")
                            .link("https://github.com/xremap/xremap")
                            .color(accent())
                            .underline(true),
                        span("."),
                    ])
                    .on_link_click(Message::OpenUrl)
                    .size(12)
                    .class(ctheme::Text::Color(muted())),
                )
                .width(Length::Fill)
                .align_x(Alignment::Center),
            )
            .push(
                container(
                    rich_text([
                        span("Downloads, source, and issue reports live on the "),
                        span("Keyloom project page")
                            .link(env!("CARGO_PKG_REPOSITORY"))
                            .color(accent())
                            .underline(true),
                        span("."),
                    ])
                    .on_link_click(Message::OpenUrl)
                    .size(12)
                    .class(ctheme::Text::Color(muted())),
                )
                .width(Length::Fill)
                .align_x(Alignment::Center),
            )
            .push(
                txt(
                    concat!(
                        "GNU General Public License version 3 only (",
                        env!("CARGO_PKG_LICENSE"),
                        ").\nProvided without warranty.",
                    ),
                    12.0,
                    muted(),
                )
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .push(
                widget::button::custom(txt_semibold("Close", 12.5, tint(0.96, 0.02)))
                    .class(accent_button())
                    .padding([9, 24])
                    .on_press(Message::CloseAbout),
            )
            .into(),
    );

    modal(card, Some(Message::CloseAbout))
}

/// Green: the setup step is in order.
fn color_ok() -> Color {
    accent()
}

/// Amber: the step has something to do, or waits for a new login.
fn color_attention() -> Color {
    oklch(0.78, 0.13, 85.0)
}

/// Red: the step cannot be fixed from here.
fn color_blocked() -> Color {
    oklch(0.62, 0.19, 25.0)
}

/// What a setup step's page says about the system.
struct StepView {
    color: Color,
    /// One-line state, also used by the summary.
    status: String,
    title: String,
    /// A sentence or two under the state.
    body: Body,
    /// What the page's main button does.
    next: Next,
    /// A second way on beside the main button that leaves the system as
    /// it is, for a page that puts a choice to the user.
    keep: Option<&'static str>,
    /// What "Show details" reveals.
    details: Details,
}

/// What a setup page's main button does. Each page has exactly one, so
/// the obvious way forward is also the one that gets the step done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    /// Nothing is known yet.
    Wait,
    /// Carry out the step's fix, and move on once it has worked. Fixes
    /// that need the administrator bring up the desktop's own password
    /// prompt.
    Fix {
        label: &'static str,
        /// The label while the fix runs.
        working: &'static str,
    },
    /// Look at the system again, once the user has changed it by hand.
    Recheck,
    /// Another step has to come first.
    Visit(Step, &'static str),
    /// Nothing is left to do here, or nothing Keyloom can do: move on.
    Continue,
}

/// A fix, as a page's main button.
const fn fix(label: &'static str, working: &'static str) -> Next {
    Next::Fix { label, working }
}

/// What a fix that asks for a password shows while the prompt is up.
const APPROVAL: &str = "Waiting for approval…";

/// Prose with at most one phrase that opens a web page.
#[derive(Default)]
enum Body {
    #[default]
    None,
    Plain(String),
    Linked {
        before: String,
        link: &'static str,
        url: &'static str,
        after: String,
    },
}

fn plain(text: impl Into<String>) -> Body {
    Body::Plain(text.into())
}

fn linked(
    before: impl Into<String>,
    link: &'static str,
    url: &'static str,
    after: impl Into<String>,
) -> Body {
    Body::Linked {
        before: before.into(),
        link,
        url,
        after: after.into(),
    }
}

/// What a step's details say: what the step changes or found, and how
/// to do it by hand, for those who want to.
#[derive(Default)]
struct Details {
    about: Body,
    /// Commands or a file to copy, or facts to read, in monospace.
    text: Option<String>,
    /// Whether `text` is meant to be copied and used.
    copyable: bool,
    /// Whether the details offer to save the service file, which the
    /// commands for turning it on by hand need in place first.
    saves_service: bool,
}

impl Details {
    fn about(about: Body) -> Self {
        Self {
            about,
            ..Self::default()
        }
    }

    /// Paths, versions, and states, to read.
    fn facts(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self.copyable = false;
        self
    }

    /// Commands (or a file) to copy.
    fn commands(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self.copyable = true;
        self
    }

    fn is_empty(&self) -> bool {
        matches!(self.about, Body::None) && self.text.is_none()
    }
}

/// The step's name in the summary.
fn step_name(step: Step) -> &'static str {
    match step {
        Step::Xremap => "xremap",
        Step::InputGroup => "Keyboard access",
        Step::Uinput => "Virtual keyboard",
        Step::Service => "Remapping",
    }
}

/// A step before the checks have landed.
fn checking(step: Step) -> StepView {
    StepView {
        color: muted(),
        status: "Checking…".to_owned(),
        title: step_name(step).to_owned(),
        body: plain("Keyloom is looking at how this system is set up."),
        next: Next::Wait,
        keep: None,
        details: Details::default(),
    }
}

/// What the page for one step says, given what the checks found.
fn step_view(facts: &Facts, step: Step) -> StepView {
    match step {
        Step::Xremap => xremap_view(facts),
        Step::InputGroup => group_view(facts),
        Step::Uinput => uinput_view(facts),
        Step::Service => service_view(facts),
    }
}

/// A path or name as one shell word, quoted only when it needs to be.
fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+:@%=,~$".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

fn xremap_view(facts: &Facts) -> StepView {
    match &facts.xremap {
        XremapCheck::Found {
            path,
            version,
            desktops,
            managed,
        } => {
            let mut detail = format!("Binary: {}", path.display());
            match (version, managed) {
                (Some(version), true) => {
                    detail.push_str(&format!("\nVersion: {version} (installed by Keyloom)"));
                }
                (Some(version), false) => detail.push_str(&format!("\nVersion: {version}")),
                (None, true) => detail.push_str("\nInstalled by Keyloom"),
                (None, false) => {}
            }
            detail.push_str(&format!("\nDesktop: {}", session_label(facts)));
            if let Some(desktops) = desktops {
                detail.push_str(&format!("\nCan ask: {}", desktop_list(desktops)));
            }
            if facts.xremap_action() == Some(XremapAction::Update) {
                if let Some(asset) = install::asset() {
                    detail.push_str(&format!(
                        "\nDownload: {}\nSHA-256: {}",
                        asset.url, asset.sha256
                    ));
                }
                return StepView {
                    color: color_attention(),
                    status: "Update available".to_owned(),
                    title: "Update xremap".to_owned(),
                    body: plain(format!(
                        "Keyloom now uses xremap {}. Updating replaces the copy Keyloom \
                         installed and restarts remapping.",
                        install::RELEASE
                    )),
                    next: fix("Update xremap", "Updating…"),
                    keep: None,
                    details: Details::about(plain(
                        "Keyloom only ever updates the copy it installed itself; xremap from \
                         your distribution is left alone.",
                    ))
                    .facts(detail),
                };
            }
            StepView {
                color: color_ok(),
                status: "Installed".to_owned(),
                title: "xremap is installed".to_owned(),
                body: matching_note(facts),
                next: Next::Continue,
                keep: None,
                details: Details::about(matching_detail(facts)).facts(detail),
            }
        }
        XremapCheck::Missing => match (facts.xremap_action(), install::asset()) {
            (Some(XremapAction::Download), Some(asset)) => {
                let destination = install::managed_path()
                    .map_or_else(String::new, |path| path.display().to_string());
                StepView {
                    color: color_attention(),
                    status: "Not installed".to_owned(),
                    title: "Install xremap".to_owned(),
                    body: plain(
                        "Keyloom remaps keys with xremap, a small open-source tool. Keyloom \
                         installs it just for you, so no password is needed.",
                    ),
                    next: fix("Install xremap", "Installing…"),
                    keep: None,
                    details: Details::about(linked(
                        format!(
                            "Keyloom downloads xremap {} from the project's releases on \
                             GitHub, makes sure it is the exact file Keyloom expects, and \
                             installs it in your home folder. To install xremap yourself \
                             instead, use your distribution's package or the ",
                            asset.version
                        ),
                        "xremap project page",
                        XREMAP_URL,
                        ", then check again.",
                    ))
                    .facts(format!(
                        "Download: {}\nSHA-256: {}\nInstalls to: {destination}",
                        asset.url, asset.sha256
                    )),
                }
            }
            _ => StepView {
                color: color_blocked(),
                status: "Not installed".to_owned(),
                title: "Install xremap".to_owned(),
                body: linked(
                    "Keyloom remaps keys with xremap, but can't install it on this system. \
                     Install it from your distribution's packages or the ",
                    "xremap project page",
                    XREMAP_URL,
                    ", then check again.",
                ),
                next: Next::Recheck,
                keep: None,
                details: Details::about(plain(format!(
                    "Keyloom looked for xremap on your PATH and in ~/.local/bin. {}",
                    if install::asset().is_none() {
                        format!(
                            "It has no xremap download for this processor ({}).",
                            std::env::consts::ARCH
                        )
                    } else {
                        "It couldn't find your home folder to install xremap into.".to_owned()
                    }
                ))),
            },
        },
    }
}

/// The session as the details name it.
fn session_label(facts: &Facts) -> String {
    let desktop = facts
        .session
        .desktop
        .map_or("not recognized", Desktop::label);
    let server = if facts.session.x11 { "X11" } else { "Wayland" };
    format!("{desktop} ({server} session)")
}

/// Desktops by name, comma-separated.
fn desktop_list(desktops: &[Desktop]) -> String {
    if desktops.is_empty() {
        return "no desktop".to_owned();
    }
    desktops
        .iter()
        .map(|desktop| desktop.label())
        .collect::<Vec<_>>()
        .join(", ")
}

/// What application-specific remaps need here, when it takes more than
/// setup does: xremap's extension on GNOME's Wayland session, or a build
/// that can ask this desktop which window is in front.
fn matching_note(facts: &Facts) -> Body {
    match facts.app_matching() {
        AppMatching::Supported(Desktop::Gnome) if !facts.session.x11 => linked(
            "Application-specific remaps on GNOME also need xremap's ",
            "GNOME Shell extension",
            XREMAP_GNOME_EXTENSION_URL,
            ", installed and turned on.",
        ),
        AppMatching::Unsupported { desktop, .. } => plain(format!(
            "This xremap can't tell which application is in front on {}, so \
             application-specific remaps won't work here.",
            desktop.label()
        )),
        AppMatching::NotInstalled
        | AppMatching::Supported(_)
        | AppMatching::Unreported
        | AppMatching::UnknownDesktop => Body::None,
    }
}

/// What the details add about application-specific remaps.
fn matching_detail(facts: &Facts) -> Body {
    match facts.app_matching() {
        AppMatching::Supported(desktop) => plain(format!(
            "This build can tell which window is in front on {}, which \
             application-specific remaps depend on.",
            desktop.label()
        )),
        AppMatching::Unsupported { supports, .. } => linked(
            if supports.is_empty() {
                "This build can't ask any desktop which window is in front. The full build \
                 from the "
                    .to_owned()
            } else {
                format!(
                    "This build can only ask {} which window is in front. The full build \
                     from the ",
                    desktop_list(&supports)
                )
            },
            "xremap project page",
            XREMAP_URL,
            " can ask every desktop.",
        ),
        AppMatching::Unreported => plain(
            "This xremap doesn't list the desktops it can ask (newer releases do), so \
             application-specific remaps depend on its build matching this desktop.",
        ),
        AppMatching::UnknownDesktop => plain(
            "Keyloom couldn't tell which desktop this is, so xremap picks on its own how to \
             ask which window is in front.",
        ),
        AppMatching::NotInstalled => Body::None,
    }
}

/// A step that only waits for a restart. Logging out is not always enough:
/// the systemd user manager that runs xremap can outlive the session and
/// keep the groups it started with.
fn after_login(title: &str, about: String) -> StepView {
    StepView {
        color: color_attention(),
        status: "Takes effect after a restart".to_owned(),
        title: title.to_owned(),
        body: plain(
            "It takes effect once you restart your computer, which you can do after setup.",
        ),
        next: Next::Continue,
        keep: None,
        details: Details::about(plain(about)),
    }
}

fn group_view(facts: &Facts) -> StepView {
    match facts.group {
        GroupCheck::Effective => StepView {
            color: color_ok(),
            status: "Allowed".to_owned(),
            title: "Keyboard access is allowed".to_owned(),
            body: Body::None,
            next: Next::Continue,
            keep: None,
            details: Details::about(plain(format!(
                "You're in the {INPUT_GROUP} group, and it's in effect for this session."
            ))),
        },
        GroupCheck::NeedsLogin => after_login(
            "Keyboard access is set up",
            format!(
                "You're in the {INPUT_GROUP} group, but this session started before you \
                 joined it."
            ),
        ),
        GroupCheck::NotMember => {
            let (body, next, user) = match facts.user.as_deref() {
                Some(user) => (
                    "Keyloom needs permission to see the keys you press. You'll be asked for \
                     your password.",
                    fix("Allow keyboard access", APPROVAL),
                    shell_word(user),
                ),
                None => (
                    "Keyloom needs permission to see the keys you press, but couldn't work out \
                     your user name to ask for it. Run the command under Show details, then \
                     check again.",
                    Next::Recheck,
                    "$USER".to_owned(),
                ),
            };
            StepView {
                color: color_attention(),
                status: "Not allowed yet".to_owned(),
                title: "Allow keyboard access".to_owned(),
                body: plain(body),
                next,
                keep: None,
                details: Details::about(plain(format!(
                    "Keyloom adds you to the {INPUT_GROUP} group, which may read every keyboard \
                     and mouse. Any program you run gets the same access, which is worth \
                     knowing on a shared computer. It takes effect once you restart your computer. \
                     To do it yourself, run this in a terminal:"
                )))
                .commands(format!("sudo usermod -aG {INPUT_GROUP} {user}")),
            }
        }
        GroupCheck::NoGroup => StepView {
            color: color_blocked(),
            status: "No input group".to_owned(),
            title: "Keyboard access isn't available".to_owned(),
            body: plain(
                "This system has no input group, which Keyloom relies on to allow keyboard \
                 access, so this step can't be done here.",
            ),
            next: Next::Continue,
            keep: None,
            details: Details::about(linked(
                format!("There's no {INPUT_GROUP} group in /etc/group. xremap's "),
                "guide to running without sudo",
                XREMAP_NO_SUDO_URL,
                " describes other ways to allow access.",
            )),
        },
    }
}

fn uinput_view(facts: &Facts) -> StepView {
    let by_hand = |situation: String| {
        Details::about(plain(format!(
            "{situation} Keyloom installs a udev rule that lets you use {UINPUT}, loads the \
             uinput module now and at every startup, and reloads udev. To do it yourself, run \
             these in a terminal:"
        )))
        .commands(uinput_commands())
    };
    let asks = || {
        plain(
            "Keyloom types your remapped keys on a virtual keyboard, which needs a one-time \
             change to the system. You'll be asked for your password.",
        )
    };
    match facts.uinput {
        UinputCheck::Writable => StepView {
            color: color_ok(),
            status: "Allowed".to_owned(),
            title: "Virtual keyboard is allowed".to_owned(),
            body: Body::None,
            next: Next::Continue,
            keep: None,
            details: Details::about(plain(format!("This session can use {UINPUT}."))),
        },
        UinputCheck::Missing { .. } => StepView {
            color: color_attention(),
            status: "Not set up".to_owned(),
            title: "Allow the virtual keyboard".to_owned(),
            body: asks(),
            next: fix("Allow virtual keyboard", APPROVAL),
            keep: None,
            details: by_hand(
                "The uinput module isn't loaded, so there's no virtual keyboard device yet."
                    .to_owned(),
            ),
        },
        UinputCheck::NotWritable {
            rule_installed: false,
        } => StepView {
            color: color_attention(),
            status: "Not allowed yet".to_owned(),
            title: "Allow the virtual keyboard".to_owned(),
            body: asks(),
            next: fix("Allow virtual keyboard", APPROVAL),
            keep: None,
            details: by_hand(format!(
                "Only the administrator can use {UINPUT} right now."
            )),
        },
        UinputCheck::NotWritable {
            rule_installed: true,
        } if facts.step_needs_login(Step::Uinput) => after_login(
            "Virtual keyboard is set up",
            format!(
                "The rule is installed ({RULES_PATH}). It lets you use {UINPUT} through the \
                 {INPUT_GROUP} group, which this session doesn't have yet."
            ),
        ),
        UinputCheck::NotWritable {
            rule_installed: true,
        } => StepView {
            color: color_attention(),
            status: "Not in effect".to_owned(),
            title: "Allow the virtual keyboard".to_owned(),
            body: plain(
                "A system rule for the virtual keyboard is installed, but this session still \
                 can't use it. Keyloom can set it up again; you'll be asked for your password.",
            ),
            next: fix("Set up again", APPROVAL),
            keep: None,
            details: by_hand(format!(
                "A rule for {UINPUT} is installed, but this session can't write to it."
            )),
        },
    }
}

fn service_view(facts: &Facts) -> StepView {
    let config = facts.config.as_ref().map_or_else(
        || "Keyloom's remaps".to_owned(),
        |path| path.display().to_string(),
    );
    let unit_path = service::unit_path().map_or_else(
        || service::UNIT.to_owned(),
        |path| path.display().to_string(),
    );
    let unit = service::UNIT;
    let installed = || Details::default().facts(format!("Service: {unit_path}\nRemaps: {config}"));
    let systemctl = |what: &str, args: &str| {
        Details::about(plain(format!("{what} To do it yourself, run:")))
            .commands(format!("systemctl --user {args} {unit}"))
    };
    let theirs = |path: Option<&Path>, exec_start: &str| {
        format!(
            "Your service: {}\nExecStart={}",
            path.map_or_else(|| unit.to_owned(), |path| path.display().to_string()),
            if exec_start.is_empty() {
                "(could not be read)"
            } else {
                exec_start
            }
        )
    };
    let installable = facts.xremap_path().is_some() && facts.config.is_some();

    match &facts.unit {
        UnitCheck::Keyloom {
            active: true,
            enabled: true,
        } => StepView {
            color: color_ok(),
            status: "Running".to_owned(),
            title: "Remapping is on".to_owned(),
            body: Body::None,
            next: Next::Continue,
            keep: None,
            details: installed(),
        },
        UnitCheck::Keyloom {
            active: false,
            enabled: true,
        } if !facts.has_effective_access() => {
            // Remapping starts by itself once xremap may use the
            // keyboards; say whether a login is all that takes.
            let login = [Step::InputGroup, Step::Uinput]
                .iter()
                .all(|step| facts.is_step_settled(*step));
            StepView {
                color: color_attention(),
                status: if login {
                    "Starts after a restart"
                } else {
                    "Waiting for keyboard access"
                }
                .to_owned(),
                title: "Remapping is set up".to_owned(),
                body: plain(if login {
                    "It starts on its own once you restart your computer."
                } else {
                    "It starts on its own once Keyloom may read your keyboard and use the \
                     virtual keyboard."
                }),
                next: Next::Continue,
                keep: None,
                details: installed(),
            }
        }
        UnitCheck::Foreign {
            reads_config: true,
            active: true,
            exec_start,
            path,
        } => StepView {
            color: color_ok(),
            status: "Running (your own service)".to_owned(),
            title: "Your xremap service works with Keyloom".to_owned(),
            body: plain(
                "It already uses Keyloom's remaps, so Keyloom leaves it alone and restarts it \
                 whenever your remaps change.",
            ),
            next: Next::Continue,
            keep: None,
            details: Details::default().facts(theirs(path.as_deref(), exec_start)),
        },
        UnitCheck::Foreign {
            reads_config: true,
            active: false,
            ..
        } => StepView {
            color: color_attention(),
            status: "Not running (your own service)".to_owned(),
            title: "Start your xremap service".to_owned(),
            body: plain(
                "It already uses Keyloom's remaps, but it isn't running. Keyloom leaves your \
                 service as it is and only starts it.",
            ),
            next: fix("Start service", "Starting…"),
            keep: None,
            details: systemctl(
                "Keyloom only starts your service; it never changes it.",
                "start",
            ),
        },
        UnitCheck::Unavailable => StepView {
            color: color_blocked(),
            status: "No systemd user session".to_owned(),
            title: "Remapping can't be turned on here".to_owned(),
            body: plain(
                "Keyloom runs remapping as a systemd user service, and this session doesn't \
                 have one.",
            ),
            next: Next::Continue,
            keep: None,
            details: match (facts.xremap_path(), facts.config.as_deref()) {
                (Some(binary), Some(config)) => Details::about(plain(
                    "You can run xremap on Keyloom's remaps yourself instead, for example from \
                     your desktop's autostart:",
                ))
                .commands(xremap_command(binary, config, facts.launch())),
                _ => Details::about(plain(
                    "Once xremap is installed, you can run it on Keyloom's remaps yourself.",
                )),
            },
        },
        // Every fix below writes Keyloom's service, which needs xremap
        // to run and a home to keep the remaps in.
        _ if !installable => {
            if facts.xremap_path().is_none() {
                StepView {
                    color: color_attention(),
                    status: "Needs xremap".to_owned(),
                    title: "Turn on remapping".to_owned(),
                    body: plain("Remapping runs through xremap, so install xremap first."),
                    next: Next::Visit(Step::Xremap, "Back to xremap"),
                    keep: None,
                    details: Details::about(plain(
                        "Keyloom's service runs xremap on your remaps, so it can only be set up \
                         once xremap is installed.",
                    )),
                }
            } else {
                StepView {
                    color: color_blocked(),
                    status: "No home folder".to_owned(),
                    title: "Remapping can't be turned on here".to_owned(),
                    body: plain("Keyloom couldn't find your home folder to keep your remaps in."),
                    next: Next::Continue,
                    keep: None,
                    details: Details::about(plain(
                        "Keyloom keeps your remaps under $XDG_CONFIG_HOME, or ~/.config when \
                         that isn't set, and neither could be found.",
                    )),
                }
            }
        }
        UnitCheck::Keyloom {
            active: true,
            enabled: false,
        } => StepView {
            color: color_attention(),
            status: "Doesn't start at login".to_owned(),
            title: "Keep remapping on".to_owned(),
            body: plain(
                "Remapping is running now, but it won't start on its own the next time you \
                 log in.",
            ),
            next: fix("Start at login", "Working…"),
            keep: None,
            details: systemctl(
                "Keyloom sets its service to start whenever you log in.",
                "enable",
            ),
        },
        UnitCheck::Keyloom {
            active: false,
            enabled: true,
        } => StepView {
            color: color_attention(),
            status: "Not running".to_owned(),
            title: "Turn remapping back on".to_owned(),
            body: plain(
                "Remapping is set up but stopped: it was paused from the header, or it failed \
                 to start.",
            ),
            next: fix("Start remapping", "Starting…"),
            keep: None,
            details: systemctl(
                &format!(
                    "Keyloom starts its service, which runs xremap on your remaps. If it fails \
                     again, journalctl --user -u {unit} shows why."
                ),
                "start",
            ),
        },
        UnitCheck::Keyloom {
            active: false,
            enabled: false,
        } => StepView {
            color: color_attention(),
            status: "Turned off".to_owned(),
            title: "Turn on remapping".to_owned(),
            body: plain("Remapping is set up but turned off, and it won't start when you log in."),
            next: fix("Turn on remapping", "Turning on…"),
            keep: None,
            // Like the fix, the command only starts xremap now where it
            // may already use the keyboards; otherwise it would fail
            // until the next login.
            details: if facts.has_effective_access() {
                systemctl(
                    "Keyloom starts its service now and whenever you log in.",
                    "enable --now",
                )
            } else {
                systemctl(
                    "Keyloom sets its service to start whenever you log in; remapping begins \
                     once keyboard access is in effect.",
                    "enable",
                )
            },
        },
        UnitCheck::Stale { .. } => StepView {
            color: color_attention(),
            status: "Needs an update".to_owned(),
            title: "Update remapping".to_owned(),
            body: plain(
                "Remapping was set up for a different xremap or desktop than this one. \
                 Updating fixes that and restarts remapping; no password is needed.",
            ),
            next: fix("Update remapping", "Updating…"),
            keep: None,
            details: Details::about(plain(format!(
                "Keyloom rewrites its service at {unit_path} for the xremap and desktop you have \
                 now, and restarts it. The service runs xremap on {config}."
            ))),
        },
        UnitCheck::Foreign {
            reads_config: false,
            exec_start,
            path,
            ..
        } => StepView {
            color: color_attention(),
            status: "Doesn't use Keyloom's remaps".to_owned(),
            title: "Replace your xremap service?".to_owned(),
            body: plain(
                "You already have an xremap service, but it doesn't use Keyloom's remaps, so \
                 changes made here won't apply. Keyloom can replace it and keep yours as a \
                 backup.",
            ),
            next: fix("Replace service", "Replacing…"),
            keep: Some("Keep mine"),
            details: Details::about(plain(format!(
                "Keeping yours leaves Keyloom's remaps unused. To keep it and use them too, add \
                 {config} to its xremap command after its own configuration file, restart it, \
                 and check again."
            )))
            .facts(theirs(path.as_deref(), exec_start)),
        },
        UnitCheck::Missing => StepView {
            color: color_attention(),
            status: "Not set up".to_owned(),
            title: "Turn on remapping".to_owned(),
            body: plain(
                "Keyloom runs xremap in the background whenever you're logged in, so your \
                 remaps work in every application. No password is needed.",
            ),
            next: fix("Turn on remapping", "Turning on…"),
            keep: None,
            details: Details {
                saves_service: true,
                ..Details::about(plain(format!(
                    "Keyloom keeps its service at {unit_path}. It runs xremap on {config}, \
                     starts whenever you log in, and restarts whenever your remaps change. To \
                     turn it on yourself, save the file first; setup then shows the command \
                     that starts it."
                )))
            },
        },
    }
}

/// The command line the service runs, for a shell.
fn xremap_command(binary: &Path, config: &Path, launch: service::Launch) -> String {
    let mut command = shell_word(&binary.to_string_lossy());
    if let Some(desktop) = launch.desktop {
        command.push_str(" --desktop ");
        command.push_str(desktop.flag());
    }
    command.push_str(" --watch ");
    command.push_str(&shell_word(&config.to_string_lossy()));
    command
}

/// Every page of setup, for the progress dots.
const SETUP_PAGES: usize = Step::ALL.len() + 2;

/// Room a setup page keeps below its content: the progress dots, the
/// buttons, and the space around them.
const SETUP_FOOTER: f32 = 84.0;

/// Padding inside the dialog cards.
const CARD_PADDING: f32 = 28.0;

fn page_index(page: SetupPage) -> usize {
    match page {
        SetupPage::Welcome => 0,
        SetupPage::Step(step) => 1 + step.index(),
        SetupPage::Finish => 1 + Step::ALL.len(),
    }
}

/// The row of progress dots under a setup page. A step setup passes
/// over, having nothing to ask, shows as done.
fn progress_dots(current: usize, facts: Option<&Facts>) -> Element<'static, Message> {
    let mut dots = widget::row::with_capacity(SETUP_PAGES)
        .spacing(10)
        .align_y(Alignment::Center);
    for index in 0..SETUP_PAGES {
        let active = index == current;
        let done = index
            .checked_sub(1)
            .and_then(|step| Step::ALL.get(step))
            .zip(facts)
            .is_some_and(|(step, facts)| !facts.wants_attention(*step));
        let color = if active {
            accent()
        } else if done {
            accent().scale_alpha(0.45)
        } else {
            white(0.16)
        };
        dots = dots.push(
            container(widget::Space::new())
                .width(Length::Fixed(if active { 18.0 } else { 6.0 }))
                .height(Length::Fixed(6.0))
                .class(ctheme::Container::custom(move |_| container::Style {
                    background: Some(color.into()),
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })),
        );
    }
    container(dots)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into()
}

/// A colored dot beside a state label.
fn status_line(color: Color, text: String) -> Element<'static, Message> {
    let dot = container(widget::Space::new().width(8.0).height(8.0)).class(
        ctheme::Container::custom(move |_| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }),
    );
    widget::row::with_capacity(2)
        .spacing(8)
        .align_y(Alignment::Center)
        .push(dot)
        .push(txt_semibold(text, 12.5, fg()))
        .into()
}

/// Prose in the muted body color, with its link when it has one.
fn prose(body: Body, size: f32) -> Option<Element<'static, Message>> {
    match body {
        Body::None => None,
        Body::Plain(text) => Some(txt(text, size, muted()).width(Length::Fill).into()),
        Body::Linked {
            before,
            link,
            url,
            after,
        } => Some(
            rich_text([
                span(before),
                span(link).link(url).color(accent()).underline(true),
                span(after),
            ])
            .on_link_click(Message::OpenUrl)
            .size(size)
            .width(Length::Fill)
            .class(ctheme::Text::Color(muted()))
            .into(),
        ),
    }
}

/// Commands, files, and paths, set apart from the prose and selectable.
fn detail_block(text: String) -> Element<'static, Message> {
    container(
        widget::selectable_text(text)
            .font(Font::MONOSPACE)
            .size(11.5)
            .class(ctheme::Text::Color(fg()))
            .width(Length::Fill)
            .wrapping(Wrapping::WordOrGlyph),
    )
    .width(Length::Fill)
    .padding([10, 12])
    .class(ctheme::Container::custom(|_| container::Style {
        background: Some(white(0.04).into()),
        border: Border {
            color: border(),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }))
    .into()
}

/// A translucent button; `None` leaves it disabled.
fn ghost(label: &str, message: Option<Message>) -> Element<'static, Message> {
    widget::button::custom(txt_semibold(
        label.to_owned(),
        12.5,
        oklch(0.85, 0.01, 152.0),
    ))
    .class(ghost_button())
    .padding([9, 16])
    .on_press_maybe(message)
    .into()
}

/// The accented call to action; `None` leaves it disabled.
fn primary(label: &str, message: Option<Message>) -> Element<'static, Message> {
    widget::button::custom(txt_semibold(label.to_owned(), 12.5, tint(0.96, 0.02)))
        .class(accent_button())
        .padding([9, 18])
        .on_press_maybe(message)
        .into()
}

/// A setup page: its content, which scrolls when the window is too
/// short for it, above the progress dots and the buttons, which stay in
/// view. `height` is what the dialog may take up.
fn setup_frame<'a>(
    content: impl Into<Element<'a, Message>>,
    page: SetupPage,
    facts: Option<&Facts>,
    buttons: impl Into<Element<'a, Message>>,
    height: f32,
) -> Element<'a, Message> {
    let room = (height - 2.0 * CARD_PADDING - SETUP_FOOTER).max(120.0);
    widget::column::with_capacity(3)
        .spacing(16)
        .push(container(widget::scrollable(content).spacing(8)).max_height(room))
        .push(progress_dots(page_index(page), facts))
        .push(buttons)
        .into()
}

/// The first-run setup wizard: a welcome page, one page per system
/// step, and a summary. Each step page's one accented button does what
/// the step needs and moves on; steps already in order are passed
/// over. Clicking outside does nothing: a stray click must not skip
/// setup, which would keep it from opening on its own again. "Set up
/// later", Finish, and Escape are the ways out.
pub fn setup_dialog<'a>(setup: &'a Setup) -> Element<'a, Message> {
    // Pages learn the height they may use, so a tall one (details in a
    // short window) scrolls instead of pushing its buttons out of view.
    let page = widget::responsive(move |space| -> Element<'a, Message> {
        let content = match setup.page {
            SetupPage::Welcome => welcome_page(setup, space.height),
            SetupPage::Step(step) => step_page(setup, step, space.height),
            SetupPage::Finish => finish_page(setup, space.height),
        };
        container(dialog_card(content)).center(Length::Fill).into()
    });
    modal(page.into(), None)
}

fn welcome_page(setup: &Setup, height: f32) -> Element<'_, Message> {
    let content = widget::column::with_capacity(3)
        .spacing(16)
        .push(eyebrow("First-run setup"))
        .push(txt_semibold("Make your keyboard your own", 24.0, fg()))
        .push(
            txt(
                "Keyloom will help you get remapping working on this system. It takes a few \
                 quick steps, and nothing changes until you say so.",
                14.0,
                muted(),
            )
            .width(Length::Fill),
        );
    let buttons = widget::row::with_capacity(3)
        .spacing(10)
        .push(ghost("Set up later", Some(Message::SetupLater)))
        .push(crate::ui::hspace())
        .push(primary("Start setup", Some(Message::SetupContinue)));
    setup_frame(
        content,
        SetupPage::Welcome,
        setup.facts.as_ref(),
        buttons,
        height,
    )
}

fn step_page(setup: &Setup, step: Step, height: f32) -> Element<'_, Message> {
    let view = setup
        .facts
        .as_ref()
        .map_or_else(|| checking(step), |facts| step_view(facts, step));
    let error = setup
        .error
        .as_ref()
        .filter(|(failed, _)| *failed == step)
        .map(|(_, error)| error);
    // Without pkexec, asking again cannot work: the way on is by hand.
    let next = match (view.next, error) {
        (Next::Fix { .. }, Some(ActionError::NoPolkit)) => Next::Recheck,
        (next, _) => next,
    };

    let mut content = widget::column::with_capacity(7)
        .spacing(16)
        .push(eyebrow(&format!(
            "First-run setup · Step {} of {}",
            step.index() + 1,
            Step::ALL.len()
        )))
        .push(txt_semibold(view.title, 24.0, fg()));
    // What the step changes, and how to do it by hand, stay behind a
    // toggle beside the status: most people never need them.
    let mut status = widget::row::with_capacity(3)
        .spacing(10)
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .push(status_line(view.color, view.status));
    if !view.details.is_empty() {
        status = status.push(crate::ui::hspace()).push(ghost(
            if setup.details {
                "Hide details"
            } else {
                "Show details"
            },
            Some(Message::SetupToggleDetails),
        ));
    }
    content = content.push(status);
    if let Some(body) = prose(view.body, 14.0) {
        content = content.push(body);
    }
    // A failure stays above the details it opens, where it is seen.
    if let Some(error) = error {
        content = content.push(
            txt(
                format!("Couldn't finish this step: {error}"),
                13.0,
                color_blocked(),
            )
            .width(Length::Fill),
        );
    }
    if setup.details && !view.details.is_empty() {
        content = content.push(details_panel(
            setup,
            step,
            next,
            view.details,
            view.keep.is_none(),
        ));
    }

    let back = step.previous().map_or(SetupPage::Welcome, SetupPage::Step);
    let mut buttons = widget::row::with_capacity(5)
        .spacing(10)
        .align_y(Alignment::Center)
        .push(ghost("Set up later", Some(Message::SetupLater)))
        .push(crate::ui::hspace())
        .push(ghost("Back", Some(Message::SetupPage(back))));
    if let Some(keep) = view.keep {
        buttons = buttons.push(ghost(
            keep,
            (!setup.is_working(step)).then_some(Message::SetupContinue),
        ));
    }
    buttons = buttons.push(main_button(setup, step, next));
    setup_frame(
        content,
        SetupPage::Step(step),
        setup.facts.as_ref(),
        buttons,
        height,
    )
}

/// The one accented button on a step page.
fn main_button(setup: &Setup, step: Step, next: Next) -> Element<'static, Message> {
    let (label, message) = match next {
        Next::Wait => ("Checking…", None),
        Next::Fix { working, .. } if setup.is_working(step) => (working, None),
        // One fix at a time.
        Next::Fix { label, .. } => (
            label,
            (!setup.is_acting()).then_some(Message::SetupAct(step)),
        ),
        Next::Recheck if setup.probing => ("Checking…", None),
        Next::Recheck => (
            "Check again",
            (!setup.is_acting()).then_some(Message::SetupRecheck),
        ),
        Next::Visit(target, label) => (label, Some(Message::SetupPage(SetupPage::Step(target)))),
        Next::Continue => ("Continue", Some(Message::SetupContinue)),
    };
    primary(label, message)
}

/// What "Show details" reveals: what the step changes, how to do it by
/// hand, a way to look again afterwards, and a way past the step for
/// those who would rather not have Keyloom do it.
fn details_panel(
    setup: &Setup,
    step: Step,
    next: Next,
    details: Details,
    can_skip: bool,
) -> Element<'static, Message> {
    let mut panel = widget::column::with_capacity(4)
        .spacing(12)
        .push(crate::ui::keyboard_view::rule(white(0.08)));
    if let Some(about) = prose(details.about, 13.0) {
        panel = panel.push(about);
    }
    let copy = details.text.clone().filter(|_| details.copyable);
    if let Some(text) = details.text {
        panel = panel.push(detail_block(text));
    }

    let mut actions = widget::row::with_capacity(5)
        .spacing(10)
        .align_y(Alignment::Center);
    if details.saves_service {
        actions = actions.push(ghost(
            if setup.saving {
                "Saving…"
            } else {
                "Save service file"
            },
            (!setup.probing && !setup.is_acting()).then_some(Message::SetupSaveService),
        ));
    }
    if let Some(text) = copy {
        actions = actions.push(ghost(
            if setup.copied { "Copied" } else { "Copy" },
            Some(Message::SetupCopy(text)),
        ));
    }
    // The main button already looks again where that is the way on.
    if next != Next::Recheck {
        actions = actions.push(ghost(
            if setup.probing {
                "Checking…"
            } else {
                "Check again"
            },
            (!setup.probing && !setup.is_acting()).then_some(Message::SetupRecheck),
        ));
    }
    if can_skip && matches!(next, Next::Fix { .. } | Next::Recheck | Next::Visit(..)) {
        actions = actions.push(crate::ui::hspace()).push(ghost(
            "Skip this step",
            (!setup.is_working(step)).then_some(Message::SetupContinue),
        ));
    }
    panel.push(actions).into()
}

/// Where an unfinished setup picks up again: the first step the user
/// can still do something about.
fn resume_step(facts: &Facts) -> Option<Step> {
    if facts.is_configured() {
        return None;
    }
    Step::ALL.into_iter().find(|step| {
        facts.wants_attention(*step)
            && matches!(
                step_view(facts, *step).next,
                Next::Fix { .. } | Next::Recheck | Next::Visit(..)
            )
    })
}

fn finish_page(setup: &Setup, height: f32) -> Element<'_, Message> {
    let facts = setup.facts.as_ref();
    let resume = facts.and_then(resume_step);
    let (title, body) = match facts {
        None => (
            "Checking your system…",
            "Keyloom is looking at how this system is set up.",
        ),
        Some(facts) if facts.is_all_ok() => (
            "You're all set",
            "Click any key on the keyboard to choose what it does. Your changes save and apply \
             on their own.",
        ),
        Some(facts) if facts.is_configured() => (
            "Almost there",
            "Restart your computer to finish. Remapping starts on its own after that, and \
             anything you set up now is kept.",
        ),
        Some(_) if resume.is_some() => (
            "Setup isn't finished",
            "Some steps still need you. Continue now, or come back any time from the ⋯ menu. \
             Anything you've set up is kept.",
        ),
        Some(_) => (
            "Setup isn't finished",
            "Some steps can't be done on this system; select one to see why. Anything you've \
             set up is kept.",
        ),
    };

    // Each step's row leads back to its page.
    let mut summary = widget::column::with_capacity(Step::ALL.len()).spacing(2);
    for step in Step::ALL {
        let view = facts.map_or_else(|| checking(step), |facts| step_view(facts, step));
        summary = summary.push(
            widget::button::custom(
                widget::row::with_capacity(4)
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .push(status_line(view.color, step_name(step).to_owned()))
                    .push(crate::ui::hspace())
                    .push(txt(view.status, 12.0, muted()))
                    .push(txt("›", 14.0, muted())),
            )
            .class(menu_row(false))
            .padding([7, 10])
            .width(Length::Fill)
            .on_press(Message::SetupPage(SetupPage::Step(step))),
        );
    }

    let mut content = widget::column::with_capacity(5)
        .spacing(16)
        .push(eyebrow("First-run setup"))
        .push(txt_semibold(title, 24.0, fg()))
        .push(txt(body, 14.0, muted()).width(Length::Fill))
        .push(summary);
    // Once remapping works, a word on what application-specific remaps
    // need beyond it, which the steps themselves pass over.
    if let Some(note) = facts
        .filter(|facts| facts.is_configured())
        .and_then(|facts| prose(matching_note(facts), 13.0))
    {
        content = content.push(note);
    }

    let buttons = widget::row::with_capacity(4)
        .spacing(10)
        .push(ghost(
            "Back",
            Some(Message::SetupPage(SetupPage::Step(Step::Service))),
        ))
        .push(crate::ui::hspace());
    // While something is left that the user can do, carrying on is the
    // way forward, and closing is the quiet alternative.
    let buttons = match resume {
        Some(step) => buttons
            .push(ghost("Set up later", Some(Message::SetupLater)))
            .push(primary(
                "Continue setup",
                Some(Message::SetupPage(SetupPage::Step(step))),
            )),
        None => buttons.push(primary("Finish", Some(Message::SetupFinish))),
    };
    setup_frame(content, SetupPage::Finish, facts, buttons, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::iced::core::{
        Event, Layout, Point, Rectangle, Shell, Size, clipboard, layout, mouse, widget::Tree,
    };

    /// Lay out `modal` in an 800×600 window and press the left mouse
    /// button at `point`, returning whether the press was captured and
    /// the messages it published.
    fn press_at(
        modal: &mut cosmic::iced::Element<'_, Message, cosmic::Theme, ()>,
        point: Point,
    ) -> (bool, Vec<Message>) {
        let mut tree = Tree::new(modal.as_widget());
        let viewport = Rectangle::with_size(Size::new(800.0, 600.0));
        let node = modal.as_widget_mut().layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        modal.as_widget_mut().update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Available(point),
            &(),
            &mut clipboard::Null,
            &mut shell,
            &viewport,
        );
        let captured = shell.is_event_captured();
        (captured, messages)
    }

    #[test]
    fn modal_captures_card_clicks_but_keeps_controls_and_backdrop_active() {
        // Use the headless renderer to exercise the actual stack's event routing.
        let control =
            mouse_area(widget::Space::new().width(80).height(40)).on_press(Message::CloseAbout);
        let card: cosmic::iced::Element<'_, Message, cosmic::Theme, ()> =
            container(control).width(200).height(120).padding(20).into();
        let mut modal = modal(card, Some(Message::CloseRemaps));

        // The centered card spans (300, 240)..(500, 360), with a control
        // at (320, 260)..(400, 300). Both padding and unused content stay open.
        for (point, expected) in [
            (Point::new(310.0, 250.0), None),
            (Point::new(450.0, 330.0), None),
            (Point::new(330.0, 270.0), Some(Message::CloseAbout)),
            (Point::new(100.0, 100.0), Some(Message::CloseRemaps)),
        ] {
            let (captured, messages) = press_at(&mut modal, point);
            assert!(captured);
            match expected {
                None => assert!(
                    messages.is_empty(),
                    "card click at {point:?} dismissed dialog"
                ),
                Some(Message::CloseAbout) => {
                    assert!(matches!(messages.as_slice(), [Message::CloseAbout]));
                }
                Some(Message::CloseRemaps) => {
                    assert!(matches!(messages.as_slice(), [Message::CloseRemaps]));
                }
                _ => unreachable!(),
            }
        }
    }

    /// Facts for every combination of the states the step pages tell
    /// apart.
    fn every_system() -> Vec<Facts> {
        use crate::session::Session;
        use std::path::PathBuf;

        let cosmic = Session {
            desktop: Some(Desktop::Cosmic),
            x11: false,
        };
        let found =
            |desktops: Option<Vec<Desktop>>, managed: bool, version: &str| XremapCheck::Found {
                path: if managed {
                    install::managed_path().unwrap_or_else(|| PathBuf::from("/xremap"))
                } else {
                    PathBuf::from("/usr/bin/xremap")
                },
                version: Some(version.to_owned()),
                desktops,
                managed,
            };
        let xremaps = [
            XremapCheck::Missing,
            found(Some(vec![Desktop::Cosmic]), false, "0.15.13"),
            found(Some(vec![Desktop::Gnome]), false, "0.15.13"),
            found(None, false, "0.15.12"),
            found(Some(Desktop::ALL.to_vec()), true, install::RELEASE),
            found(Some(Desktop::ALL.to_vec()), true, "0.15.12"),
        ];
        let groups = [
            GroupCheck::Effective,
            GroupCheck::NeedsLogin,
            GroupCheck::NotMember,
            GroupCheck::NoGroup,
        ];
        let uinputs = [
            UinputCheck::Writable,
            UinputCheck::Missing {
                rule_installed: false,
            },
            UinputCheck::NotWritable {
                rule_installed: false,
            },
            UinputCheck::NotWritable {
                rule_installed: true,
            },
        ];
        let foreign = |reads_config, active| UnitCheck::Foreign {
            exec_start: "/usr/bin/xremap /home/me/mine.yml".to_owned(),
            reads_config,
            active,
            path: Some(PathBuf::from(
                "/home/me/.config/systemd/user/xremap.service",
            )),
        };
        let units = [
            UnitCheck::Keyloom {
                active: true,
                enabled: true,
            },
            UnitCheck::Keyloom {
                active: true,
                enabled: false,
            },
            UnitCheck::Keyloom {
                active: false,
                enabled: true,
            },
            UnitCheck::Keyloom {
                active: false,
                enabled: false,
            },
            UnitCheck::Stale { active: true },
            foreign(true, true),
            foreign(true, false),
            foreign(false, true),
            UnitCheck::Missing,
            UnitCheck::Unavailable,
        ];

        let mut systems = Vec::new();
        for xremap in &xremaps {
            for group in groups {
                for uinput in uinputs {
                    for unit in &units {
                        for user in [Some("me"), None] {
                            for config in [Some("/home/me/.config/xremap/keyloom.yml"), None] {
                                systems.push(Facts {
                                    user: user.map(str::to_owned),
                                    xremap: xremap.clone(),
                                    group,
                                    uinput,
                                    unit: unit.clone(),
                                    config: config.map(PathBuf::from),
                                    session: cosmic,
                                });
                            }
                        }
                    }
                }
            }
        }
        systems
    }

    #[test]
    fn a_page_offers_exactly_the_fix_setup_would_run() {
        for facts in every_system() {
            for step in Step::ALL {
                let view = step_view(&facts, step);
                assert_eq!(
                    matches!(view.next, Next::Fix { .. }),
                    facts.can_fix(step),
                    "{step:?} on {facts:?}"
                );
                // A page setup passes over must agree there is nothing
                // left to do on it.
                if !facts.wants_attention(step) {
                    assert_eq!(view.next, Next::Continue, "{step:?} on {facts:?}");
                    assert!(view.keep.is_none());
                }
                // The only choice beside the fix is keeping a service.
                if view.keep.is_some() {
                    assert!(matches!(view.next, Next::Fix { .. }));
                }
                assert!(!view.title.is_empty() && !view.status.is_empty());
            }
        }
    }

    #[test]
    fn a_page_that_needs_the_user_to_act_by_hand_says_how() {
        for facts in every_system() {
            for step in Step::ALL {
                let view = step_view(&facts, step);
                match view.next {
                    // Anything left to do comes with details, which say
                    // how to do it by hand and hold the way past it.
                    Next::Fix { .. } | Next::Recheck => {
                        assert!(!view.details.is_empty(), "{step:?} on {facts:?}");
                    }
                    Next::Visit(target, _) => {
                        assert_ne!(target, step);
                        assert!(!view.details.is_empty(), "{step:?} on {facts:?}");
                    }
                    Next::Wait | Next::Continue => {}
                }
                // Commands to copy are only ever offered as commands.
                if view.details.copyable {
                    assert!(view.details.text.is_some());
                }
            }
        }
    }

    #[test]
    fn an_unfinished_summary_picks_up_where_the_user_can_act() {
        for facts in every_system() {
            match resume_step(&facts) {
                Some(step) => {
                    assert!(!facts.is_configured());
                    assert!(facts.wants_attention(step));
                    // Nothing earlier was left for the user to do.
                    for earlier in &Step::ALL[..step.index()] {
                        assert!(
                            !facts.wants_attention(*earlier)
                                || step_view(&facts, *earlier).next == Next::Continue,
                            "{earlier:?} on {facts:?}"
                        );
                    }
                }
                // A finished summary, or one whose open steps are all
                // beyond the user's reach, only closes.
                None => assert!(
                    facts.is_configured()
                        || Step::ALL.iter().all(|step| {
                            !facts.wants_attention(*step)
                                || step_view(&facts, *step).next == Next::Continue
                        }),
                    "{facts:?}"
                ),
            }
        }
    }

    #[test]
    fn turning_the_service_on_by_hand_saves_the_file_before_any_command() {
        let unit_path = service::unit_path().map_or_else(
            || service::UNIT.to_owned(),
            |path| path.display().to_string(),
        );
        let systems = every_system();
        for facts in &systems {
            let details = step_view(facts, Step::Service).details;
            // Saving is offered exactly where the file is missing and
            // could be written, and never beside commands that would
            // need it in place.
            assert_eq!(details.saves_service, facts.can_save_service(), "{facts:?}");
            if details.saves_service {
                assert!(details.text.is_none(), "{facts:?}");
                let Body::Plain(about) = &details.about else {
                    panic!("the service step explains itself in plain text");
                };
                assert!(about.contains(&unit_path), "{about}");
                let config = facts.config.as_ref().expect("a configuration to run");
                assert!(about.contains(&*config.to_string_lossy()), "{about}");
            }
            // Any systemctl command shown acts on a unit that exists.
            if let Some(text) = &details.text
                && text.contains("systemctl")
            {
                assert!(
                    !matches!(facts.unit, UnitCheck::Missing | UnitCheck::Unavailable),
                    "{facts:?}"
                );
            }
        }

        // Once saved, the page shows the command that turns it on, which
        // starts xremap right away only where it may use the keyboards.
        let saved = UnitCheck::Keyloom {
            active: false,
            enabled: false,
        };
        for (access, command) in [
            (true, "systemctl --user enable --now xremap.service"),
            (false, "systemctl --user enable xremap.service"),
        ] {
            let facts = systems
                .iter()
                .find(|facts| {
                    facts.unit == saved
                        && facts.has_effective_access() == access
                        && facts.can_fix(Step::Service)
                })
                .expect("a system with the file saved");
            let details = step_view(facts, Step::Service).details;
            assert!(details.copyable);
            assert_eq!(details.text.as_deref(), Some(command));
            assert!(!details.saves_service);
        }
    }

    #[test]
    fn manual_commands_quote_what_a_shell_would_split() {
        assert_eq!(shell_word("/usr/bin/xremap"), "/usr/bin/xremap");
        assert_eq!(shell_word("$USER"), "$USER");
        assert_eq!(
            shell_word("/home/Jo Doe/.config/xremap/keyloom.yml"),
            "'/home/Jo Doe/.config/xremap/keyloom.yml'"
        );
        assert_eq!(shell_word("it's"), r"'it'\''s'");
        assert_eq!(shell_word(""), "''");
        assert_eq!(
            xremap_command(
                Path::new("/usr/bin/xremap"),
                Path::new("/home/Jo Doe/keyloom.yml"),
                service::Launch {
                    desktop: Some(Desktop::Cosmic),
                    wait_for_wayland: true,
                },
            ),
            "/usr/bin/xremap --desktop cosmic --watch '/home/Jo Doe/keyloom.yml'"
        );
    }

    #[test]
    fn inert_backdrop_swallows_clicks_without_dismissing() {
        // Setup passes no backdrop message: a stray click outside the card
        // must neither close the dialog nor fall through to the page.
        let card: cosmic::iced::Element<'_, Message, cosmic::Theme, ()> =
            container(widget::Space::new())
                .width(200)
                .height(120)
                .into();
        let mut modal = modal(card, None);

        let (captured, messages) = press_at(&mut modal, Point::new(100.0, 100.0));
        assert!(captured, "backdrop click fell through to the page");
        assert!(messages.is_empty(), "backdrop click dismissed the dialog");
    }
}

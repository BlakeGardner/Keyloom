//! Popover panels, the confirmation toast, and modal dialogs.

use cosmic::iced::core::text::Wrapping;
use cosmic::iced::widget::{rich_text, span};
use cosmic::iced::{Alignment, Border, Color, Length, Padding};
use cosmic::widget::{self, container, icon, mouse_area};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message, Picker, PickerTarget, Setup, SetupPage, Toast, View};
use crate::apps;
use crate::keyboard;
use crate::service;
use crate::setup::{
    Facts, GroupCheck, INPUT_GROUP, RULE, RULES_PATH, Step, UINPUT, UinputCheck, UnitCheck,
    XREMAP_URL, XremapCheck,
};
use crate::ui::model::key_name;
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
    for (index, (code, mapping)) in maps.iter().enumerate() {
        if crate::ui::model::key(code).is_none() {
            continue;
        }
        let mut full_to = if mapping.normal {
            format!("{} · normal here", key_name(code))
        } else {
            mapping.tap.clone().unwrap_or_else(|| key_name(code))
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
                                    .push(mono(key_name(code), 13.0, fg()))
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
        widget::column::with_capacity(3)
            .spacing(16)
            .push(header)
            .push(description)
            .push(
                // Reserve a scrollbar gutter so it cannot cover the remove buttons
                // or intercept their hover and click events.
                container(widget::scrollable(rows).spacing(8))
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

    modal(card, Some(Message::CloseRemaps))
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

    modal(card, Some(Message::SetCapture(false)))
}

/// Confirm removal while keeping the remaps list open underneath.
pub fn remove_mapping_dialog(app: &App) -> Element<'_, Message> {
    let entry = app
        .confirm_remove_mapping
        .and_then(|index| app.maps().get(index));
    let name = entry.map_or_else(|| "this key".to_owned(), |(code, _)| key_name(code));
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
            let key = key_name(&layer.trigger);
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
        include_bytes!("../../assets/keyloom_logo.svg").as_slice(),
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
                txt("Built with Rust and libcosmic. Powered by xremap.", 12.0, muted())
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .push(
                // Centered through the container, not the text: rich text
                // places its link spans as if it were left-aligned.
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
    /// Explanation under the state.
    body: Body,
    /// Paths, units, and commands, shown only on request.
    detail: Option<String>,
    /// The fix the step's button carries out, if it has one. Fixes
    /// that need the administrator bring up the desktop's own password
    /// prompt.
    action: Option<&'static str>,
}

/// The explanation under a step's status.
enum Body {
    /// The status says it all.
    None,
    Plain(String),
    /// Text with one phrase that opens a web page.
    Linked {
        before: String,
        link: &'static str,
        url: &'static str,
        after: String,
    },
}

/// The fix a step offers, as its button label.
const fn fix(label: &'static str) -> Option<&'static str> {
    Some(label)
}

/// The step's name in the summary.
fn step_name(step: Step) -> &'static str {
    match step {
        Step::Xremap => "xremap",
        Step::InputGroup => "Keyboard access",
        Step::Uinput => "Virtual keyboard",
        Step::Service => "Remapping service",
    }
}

/// A step before the checks have landed.
fn checking(step: Step) -> StepView {
    StepView {
        color: muted(),
        status: "Checking…".to_owned(),
        title: step_name(step).to_owned(),
        body: Body::Plain("Keyloom is looking at how this system is set up.".to_owned()),
        detail: None,
        action: None,
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

fn xremap_view(facts: &Facts) -> StepView {
    match &facts.xremap {
        XremapCheck::Found { path, version } => StepView {
            color: color_ok(),
            status: match version {
                Some(version) => format!("Installed (version {version})"),
                None => "Installed".to_owned(),
            },
            title: "xremap is installed".to_owned(),
            body: Body::None,
            detail: Some(format!("Binary: {}", path.display())),
            action: None,
        },
        XremapCheck::Missing => StepView {
            color: color_blocked(),
            status: "Not installed".to_owned(),
            title: "Install xremap first".to_owned(),
            body: Body::Linked {
                before: "Keyloom does its remapping through xremap, and it isn't installed \
                         yet. Install it from your distribution's packages or from the "
                    .to_owned(),
                link: "xremap project page",
                url: XREMAP_URL,
                after: ", then check again. You can keep mapping keys in the meantime.".to_owned(),
            },
            detail: Some(format!(
                "Looked for an executable named xremap on PATH.\n{XREMAP_URL}"
            )),
            action: None,
        },
    }
}

fn group_view(facts: &Facts) -> StepView {
    let user = facts.user.as_deref();
    let command = format!("usermod -aG {INPUT_GROUP} {}", user.unwrap_or("$USER"));
    match facts.group {
        GroupCheck::Effective => StepView {
            color: color_ok(),
            status: "Member of the input group".to_owned(),
            title: "Keyloom can read your keyboard".to_owned(),
            body: Body::None,
            detail: Some(format!(
                "Group: {INPUT_GROUP} (membership in effect for this session)"
            )),
            action: None,
        },
        GroupCheck::NeedsLogin => StepView {
            color: color_attention(),
            status: "Takes effect at your next login".to_owned(),
            title: "Log out and back in".to_owned(),
            body: Body::Plain(
                "You've been given permission to read keyboards, but this session started \
                   before that. Log out and back in (or restart) and remapping starts on its \
                   own afterwards."
                    .to_owned(),
            ),
            detail: Some(format!(
                "Group: {INPUT_GROUP} (listed, but not in this session's groups yet)\n\
                 Applied: {command}"
            )),
            action: None,
        },
        GroupCheck::NotMember => StepView {
            color: color_attention(),
            status: "No permission yet".to_owned(),
            title: "Let Keyloom read your keyboard".to_owned(),
            body: Body::Plain(format!(
                "Keyloom needs permission to read your keyboard so xremap can see the keys \
                 you press. Any program running as you gains the same ability, which is worth \
                 knowing on a shared computer. The change takes effect the next time you log \
                 in.{}",
                if user.is_none() {
                    " Keyloom could not work out your user name, so run the command in the \
                     details yourself."
                } else {
                    ""
                }
            )),
            detail: Some(format!(
                "Adds {} to the {INPUT_GROUP} group, which may read /dev/input:\n{command}",
                user.unwrap_or("your user")
            )),
            action: user.and(fix("Add me to the input group")),
        },
        GroupCheck::NoGroup => StepView {
            color: color_blocked(),
            status: "Can't be set up here".to_owned(),
            title: "This system has no input group".to_owned(),
            body: Body::Plain(
                "Keyboards are normally readable by a group that the system creates. \
                   Without it, Keyloom cannot grant access; xremap's guide to running \
                   without sudo describes the manual steps."
                    .to_owned(),
            ),
            detail: Some(format!("No {INPUT_GROUP} group in /etc/group.")),
            action: None,
        },
    }
}

fn uinput_view(facts: &Facts) -> StepView {
    let rule = format!("Rule: {RULE}\n→ {RULES_PATH}");
    match facts.uinput {
        UinputCheck::Writable => StepView {
            color: color_ok(),
            status: "Ready".to_owned(),
            title: "Keyloom can create a virtual keyboard".to_owned(),
            body: Body::None,
            detail: Some(format!("Device: {UINPUT} (writable from this session)")),
            action: None,
        },
        UinputCheck::Missing { .. } => StepView {
            color: color_attention(),
            status: "Not enabled".to_owned(),
            title: "Enable the virtual keyboard".to_owned(),
            body: Body::Plain(
                "Remapped keys are typed on a virtual keyboard, and this system hasn't \
                   enabled the device for it yet. Keyloom can enable it now and keep it \
                   available after every restart."
                    .to_owned(),
            ),
            detail: Some(format!(
                "Device: {UINPUT} (missing: the uinput module is not loaded)\n\
                 Loads the module, registers it in /etc/modules-load.d/uinput.conf, \
                 and installs the udev rule.\n{rule}"
            )),
            action: fix("Set up the virtual keyboard"),
        },
        UinputCheck::NotWritable {
            rule_installed: false,
        } => StepView {
            color: color_attention(),
            status: "No permission yet".to_owned(),
            title: "Allow access to the virtual keyboard".to_owned(),
            body: Body::Plain(
                "Remapped keys are typed on a virtual keyboard, and only the administrator \
                   may use it right now. A small system rule grants you access."
                    .to_owned(),
            ),
            detail: Some(format!(
                "Device: {UINPUT} (not writable)\nInstalls the udev rule and reloads \
                 udev.\n{rule}"
            )),
            action: fix("Install the udev rule"),
        },
        UinputCheck::NotWritable {
            rule_installed: true,
        } => StepView {
            color: color_attention(),
            status: "Rule installed, not in effect yet".to_owned(),
            title: "Access isn't in effect yet".to_owned(),
            body: Body::Plain(
                "The access rule is installed, but this session can't use the virtual \
                   keyboard yet. That usually takes effect at your next login; you can also \
                   apply the rule again now."
                    .to_owned(),
            ),
            detail: Some(format!("Device: {UINPUT} (not writable yet)\n{rule}")),
            action: fix("Apply the rule again"),
        },
    }
}

fn service_view(facts: &Facts) -> StepView {
    let config = facts.config.as_ref().map_or_else(
        || "its configuration".to_owned(),
        |path| path.display().to_string(),
    );
    let unit_path = service::unit_path().map_or_else(
        || service::UNIT.to_owned(),
        |path| path.display().to_string(),
    );
    // The unit Keyloom would install, as the user will find it.
    let planned = || {
        let (Some(binary), Some(config)) = (facts.xremap_path(), facts.config.as_deref()) else {
            return None;
        };
        Some(format!(
            "Unit: {unit_path}\nExecStart={}",
            crate::setup::exec_start_of(&service::unit_file(binary, config))
        ))
    };
    let installed = || Some(format!("Unit: {unit_path}\nRemaps: {config}"));
    match &facts.unit {
        UnitCheck::Keyloom {
            active: true,
            enabled: true,
        } => StepView {
            color: color_ok(),
            status: "Running".to_owned(),
            title: "Remapping is running".to_owned(),
            body: Body::None,
            detail: installed(),
            action: None,
        },
        UnitCheck::Keyloom {
            active: true,
            enabled: false,
        } => StepView {
            color: color_attention(),
            status: "Running, but not at login".to_owned(),
            title: "Start remapping at login".to_owned(),
            body: Body::Plain(
                "Remapping is running now, but it won't start on its own the next time \
                   you log in."
                    .to_owned(),
            ),
            detail: installed(),
            action: fix("Enable at login"),
        },
        UnitCheck::Keyloom {
            active: false,
            enabled: true,
        } if !facts.has_effective_access() => StepView {
            color: color_attention(),
            status: "Starts after your next login".to_owned(),
            title: "Remapping is ready".to_owned(),
            body: Body::Plain(
                "Everything is installed. Remapping starts on its own once you've logged \
                   out and back in."
                    .to_owned(),
            ),
            detail: installed(),
            action: None,
        },
        UnitCheck::Keyloom {
            active: false,
            enabled: true,
        } => StepView {
            color: color_attention(),
            status: "Not running".to_owned(),
            title: "Remapping isn't running".to_owned(),
            body: Body::Plain(
                "Remapping is installed but stopped: paused from the header, or it failed \
                   to start. Starting it applies your remaps."
                    .to_owned(),
            ),
            detail: installed(),
            action: fix("Start remapping"),
        },
        UnitCheck::Keyloom {
            active: false,
            enabled: false,
        } => StepView {
            color: color_attention(),
            status: "Installed but turned off".to_owned(),
            title: "Turn remapping on".to_owned(),
            body: Body::Plain(
                "Remapping is installed but neither running nor set to start when you log \
                   in."
                .to_owned(),
            ),
            detail: installed(),
            action: fix("Enable and start"),
        },
        UnitCheck::Stale { .. } => StepView {
            color: color_attention(),
            status: "Needs updating".to_owned(),
            title: "Update remapping".to_owned(),
            body: Body::Plain(
                "The way remapping was set up doesn't match this version of Keyloom. \
                   Updating rewrites it and restarts remapping."
                    .to_owned(),
            ),
            detail: planned(),
            action: fix("Update service"),
        },
        UnitCheck::Foreign {
            reads_config: true,
            active,
            exec_start,
            path,
            ..
        } => StepView {
            color: if *active {
                color_ok()
            } else {
                color_attention()
            },
            status: if *active {
                "Running (set up by you)".to_owned()
            } else {
                "Not running (set up by you)".to_owned()
            },
            title: if *active {
                "Your own setup already works with Keyloom".to_owned()
            } else {
                "Start your remapping service".to_owned()
            },
            body: Body::Plain(
                "A remapping service you set up yourself already applies Keyloom's remaps, \
                   so Keyloom leaves it alone and only restarts it when your remaps change."
                    .to_owned(),
            ),
            detail: Some(format!(
                "Unit: {}\nExecStart={exec_start}",
                path.as_ref().map_or_else(
                    || service::UNIT.to_owned(),
                    |path| path.display().to_string()
                )
            )),
            action: if *active { None } else { fix("Start service") },
        },
        UnitCheck::Foreign {
            reads_config: false,
            exec_start,
            path,
            ..
        } => StepView {
            color: color_attention(),
            status: "Doesn't use Keyloom's remaps".to_owned(),
            title: "An existing setup doesn't use Keyloom's remaps".to_owned(),
            body: Body::Plain(
                "A remapping service that Keyloom didn't set up already exists, and it \
                   doesn't use Keyloom's remaps, so mappings made here have no effect. \
                   Replacing it installs Keyloom's own (the existing file is backed up beside \
                   it), but whatever it runs now stops being applied. Leave it if you'd \
                   rather manage xremap yourself."
                    .to_owned(),
            ),
            detail: Some(format!(
                "Unit: {}\nExecStart={}\nKeyloom's remaps: {config}",
                path.as_ref().map_or_else(
                    || service::UNIT.to_owned(),
                    |path| path.display().to_string()
                ),
                if exec_start.is_empty() {
                    "(could not be read)"
                } else {
                    exec_start
                }
            )),
            action: facts.service_action().and(fix("Replace service")),
        },
        UnitCheck::Missing => StepView {
            color: color_attention(),
            status: "Not installed".to_owned(),
            title: "Set up remapping".to_owned(),
            body: Body::Plain(format!(
                "Keyloom installs a small background service that applies your remaps, \
                 starts when you log in, and restarts whenever a mapping changes. No \
                 password is needed.{}",
                if facts.service_action().is_none() {
                    " Install xremap first."
                } else {
                    ""
                }
            )),
            detail: planned(),
            action: facts.service_action().and(fix("Install service")),
        },
        UnitCheck::Unavailable => StepView {
            color: color_blocked(),
            status: "Can't be set up here".to_owned(),
            title: "Keyloom can't manage remapping here".to_owned(),
            body: Body::Plain(
                "Keyloom runs remapping as a background service through systemd, and this \
                   session has none."
                    .to_owned(),
            ),
            detail: Some(format!(
                "No systemd user session. You can run xremap yourself on {config}."
            )),
            action: None,
        },
    }
}

/// Every page of setup, for the progress dots.
const SETUP_PAGES: usize = Step::ALL.len() + 2;

fn page_index(page: SetupPage) -> usize {
    match page {
        SetupPage::Welcome => 0,
        SetupPage::Step(step) => 1 + step.index(),
        SetupPage::Finish => 1 + Step::ALL.len(),
    }
}

/// The row of progress dots under a setup page.
fn progress_dots(current: usize) -> Element<'static, Message> {
    let mut dots = widget::row::with_capacity(SETUP_PAGES)
        .spacing(10)
        .align_y(Alignment::Center);
    for index in 0..SETUP_PAGES {
        let active = index == current;
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

/// Commands and paths, set apart from the prose.
fn detail_block(text: String) -> Element<'static, Message> {
    container(
        mono(text, 11.5, fg())
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

/// The first-run setup wizard: a welcome page, one page per system
/// step, and a summary. Clicking outside does nothing: a stray click
/// must not skip setup, which would keep it from opening on its own
/// again. "Skip for now", Finish, and Escape are the ways out.
pub fn setup_dialog(setup: &Setup) -> Element<'_, Message> {
    let content = match setup.page {
        SetupPage::Welcome => welcome_page(),
        SetupPage::Step(step) => step_page(setup, step),
        SetupPage::Finish => finish_page(setup),
    };
    modal(dialog_card(content), None)
}

fn welcome_page() -> Element<'static, Message> {
    widget::column::with_capacity(6)
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
        )
        .push(progress_dots(0))
        .push(
            widget::row::with_capacity(3)
                .spacing(10)
                .push(ghost("Skip for now", Some(Message::SetupSkip)))
                .push(crate::ui::hspace())
                .push(primary(
                    "Start setup",
                    Some(Message::SetupPage(SetupPage::Step(Step::Xremap))),
                )),
        )
        .into()
}

fn step_page(setup: &Setup, step: Step) -> Element<'_, Message> {
    let view = setup
        .facts
        .as_ref()
        .map_or_else(|| checking(step), |facts| step_view(facts, step));
    let busy = setup.busy == Some(step);

    let mut column = widget::column::with_capacity(9)
        .spacing(16)
        .push(eyebrow(&format!(
            "First-run setup · Step {} of {}",
            step.index() + 1,
            Step::ALL.len()
        )))
        .push(txt_semibold(view.title, 24.0, fg()));
    // Paths, units, and commands stay behind a toggle beside the
    // status: most people never need them, and the page reads better
    // without them.
    let mut status = widget::row::with_capacity(3)
        .spacing(10)
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .push(status_line(view.color, view.status));
    if view.detail.is_some() {
        status = status.push(crate::ui::hspace()).push(ghost(
            if setup.details {
                "Hide details"
            } else {
                "Show details"
            },
            Some(Message::SetupToggleDetails),
        ));
    }
    column = column.push(status);
    match view.body {
        Body::None => {}
        Body::Plain(text) => {
            column = column.push(txt(text, 14.0, muted()).width(Length::Fill));
        }
        Body::Linked {
            before,
            link,
            url,
            after,
        } => {
            column = column.push(
                rich_text([
                    span(before),
                    span(link).link(url).color(accent()).underline(true),
                    span(after),
                ])
                .on_link_click(Message::OpenUrl)
                .size(14)
                .width(Length::Fill)
                .class(ctheme::Text::Color(muted())),
            );
        }
    }
    if setup.details
        && let Some(detail) = view.detail
    {
        column = column.push(detail_block(detail));
    }
    if let Some((_, error)) = setup.error.as_ref().filter(|(failed, _)| *failed == step) {
        column = column.push(
            txt(
                format!("Could not finish this step: {error}"),
                13.0,
                color_blocked(),
            )
            .width(Length::Fill),
        );
    }

    // The fix on offer, then a way to look again after doing something
    // by hand. Nothing else shares this row, so it cannot outgrow the
    // card.
    let mut actions = widget::row::with_capacity(2)
        .spacing(10)
        .align_y(Alignment::Center);
    if let Some(label) = view.action {
        actions = actions.push(primary(
            if busy { "Working…" } else { label },
            (!busy).then_some(Message::SetupAct(step)),
        ));
    }
    actions = actions.push(ghost(
        if setup.probing {
            "Checking…"
        } else {
            "Check again"
        },
        (!setup.probing).then_some(Message::SetupRecheck),
    ));
    column = column.push(actions);

    column = column.push(progress_dots(page_index(SetupPage::Step(step))));
    let back = step.previous().map_or(SetupPage::Welcome, SetupPage::Step);
    let next = step.next().map_or(SetupPage::Finish, SetupPage::Step);
    column
        .push(
            widget::row::with_capacity(4)
                .spacing(10)
                .push(ghost("Skip for now", Some(Message::SetupSkip)))
                .push(crate::ui::hspace())
                .push(ghost("Back", Some(Message::SetupPage(back))))
                .push(primary("Continue", Some(Message::SetupPage(next)))),
        )
        .into()
}

fn finish_page(setup: &Setup) -> Element<'_, Message> {
    let facts = setup.facts.as_ref();
    let (title, body) = match facts {
        None => (
            "Checking your system…",
            "Keyloom is looking at how this system is set up.",
        ),
        Some(facts) if facts.is_all_ok() => (
            "You're all set",
            "Click any key on the keyboard to choose what it should do. Mappings save and \
             apply on their own, and the Tester shows what your keyboard sends.",
        ),
        Some(facts) if facts.is_configured() => (
            "Almost there",
            "Log out and back in to finish. Remapping starts on its own afterwards, and \
             mappings you make now are kept.",
        ),
        Some(_) => (
            "Setup isn't finished",
            "Some steps still need attention. Go back to them now, or return to setup any \
             time from the ⋯ menu or the remapping status in the header. Mappings you make \
             are saved and apply once remapping runs.",
        ),
    };

    let mut summary = widget::column::with_capacity(Step::ALL.len()).spacing(8);
    for step in Step::ALL {
        let view = facts.map_or_else(|| checking(step), |facts| step_view(facts, step));
        summary = summary.push(
            widget::row::with_capacity(3)
                .spacing(10)
                .align_y(Alignment::Center)
                .push(status_line(view.color, step_name(step).to_owned()))
                .push(crate::ui::hspace())
                .push(txt(view.status, 12.0, muted())),
        );
    }

    widget::column::with_capacity(6)
        .spacing(16)
        .push(eyebrow("First-run setup"))
        .push(txt_semibold(title, 24.0, fg()))
        .push(txt(body, 14.0, muted()).width(Length::Fill))
        .push(container(summary).padding([4, 0]))
        .push(progress_dots(page_index(SetupPage::Finish)))
        .push(
            widget::row::with_capacity(3)
                .spacing(10)
                .push(ghost(
                    "Back",
                    Some(Message::SetupPage(SetupPage::Step(Step::Service))),
                ))
                .push(crate::ui::hspace())
                .push(primary("Finish", Some(Message::SetupFinish))),
        )
        .into()
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

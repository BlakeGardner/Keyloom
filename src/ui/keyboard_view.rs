//! The keyboard workspace: device toolbar, layer picker, the layer bar,
//! the rendered deck of key caps, and the mapping summary underneath.

use cosmic::iced::widget::stack;
use cosmic::iced::{Alignment, Background, Border, Color, Font, Length, Padding, font::Weight};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message, Popover, View};
use crate::keyboard;
use crate::ui::model::{self, key_name, short};
use crate::ui::theme::{
    ButtonStyle, accent, accent_button, chip, fg, ghost_button, muted, oklch, tint, vgradient,
    white,
};
use crate::ui::{eyebrow, panel, tester, txt, txt_semibold};
use crate::xremap;

/// The device scope (or tester input filter) and the layer picker.
pub fn device_toolbar(app: &App) -> Element<'_, Message> {
    let device = widget::button::custom(
        widget::row::with_capacity(3)
            .spacing(9)
            .align_y(Alignment::Center)
            .push(txt(
                if app.view == View::Tester {
                    "Listen to"
                } else {
                    "Applies to"
                },
                11.0,
                muted(),
            ))
            .push(txt_semibold(
                app.device_label(&app.device),
                12.0,
                oklch(0.93, 0.01, 152.0),
            ))
            .push(txt("▾", 9.0, white(0.4))),
    )
    .class(
        ButtonStyle {
            bg: Some(Background::Color(white(0.035))),
            hover_bg: Some(Background::Color(white(0.075))),
            border: white(0.09),
            border_width: 1.0,
            radius: 9.0,
            ..ButtonStyle::default()
        }
        .class(),
    )
    .padding([8, 12])
    .on_press(Message::TogglePopover(Popover::Devices));

    let device: Element<'_, Message> = if app.popover == Some(Popover::Devices) {
        widget::popover(device)
            .popup(crate::ui::overlays::devices_popup(app))
            .position(widget::popover::Position::Bottom)
            .on_close(Message::CloseOverlays)
            .into()
    } else {
        device.into()
    };

    // The size & layout picker, defaulted by detection.
    let size = widget::button::custom(
        widget::row::with_capacity(3)
            .spacing(9)
            .align_y(Alignment::Center)
            .push(txt("Size", 11.0, muted()))
            .push(txt_semibold(
                format!(
                    "{} · {}",
                    keyboard::FORM_FACTORS[app.form].name,
                    if app.iso { "ISO" } else { "ANSI" }
                ),
                12.0,
                oklch(0.93, 0.01, 152.0),
            ))
            .push(txt("▾", 9.0, white(0.4))),
    )
    .class(
        ButtonStyle {
            bg: Some(Background::Color(white(0.035))),
            hover_bg: Some(Background::Color(white(0.075))),
            border: white(0.09),
            border_width: 1.0,
            radius: 9.0,
            ..ButtonStyle::default()
        }
        .class(),
    )
    .padding([8, 12])
    .on_press(Message::TogglePopover(Popover::Size));

    let size: Element<'_, Message> = if app.popover == Some(Popover::Size) {
        widget::popover(size)
            .popup(crate::ui::overlays::size_popup(app))
            .position(widget::popover::Position::Bottom)
            .on_close(Message::CloseOverlays)
            .into()
    } else {
        size.into()
    };

    // Layers are edited on the deck, so the tester has no use for them.
    let mut layers = widget::column::with_capacity(3)
        .spacing(8)
        .align_x(Alignment::End);
    if app.view == View::Keyboard {
        layers = layers.push(
            widget::button::custom(txt(
                if app.layers_open {
                    "▾ Layers"
                } else {
                    "▸ Layers"
                },
                13.0,
                fg(),
            ))
            .class(ctheme::Button::Transparent)
            .padding([8, 4])
            .on_press(Message::ToggleLayers),
        );
        if app.layers_open {
            layers = layers.push(txt(
                "Hold one key to give the others a second job. Keys without one keep working normally.",
                13.0,
                muted(),
            ));
            layers = layers.push(layer_chips(app));
        }
    }

    widget::column::with_capacity(2)
        .push(
            widget::row::with_capacity(4)
                .spacing(10)
                .padding(Padding {
                    top: 12.0,
                    right: 30.0,
                    bottom: 12.0,
                    left: 30.0,
                })
                .align_y(Alignment::Start)
                .push(device)
                .push(size)
                .push(crate::ui::hspace())
                .push(layers),
        )
        .push(rule(white(0.05)))
        .into()
}

/// The layer picker chips: the normal keys, each layer of the profile,
/// and the way to add one.
fn layer_chips(app: &App) -> Element<'_, Message> {
    let mut chips: Vec<Element<'_, Message>> = Vec::with_capacity(app.layers().len() + 2);
    let normal = app.layer.is_none();
    chips.push(
        widget::button::custom(txt_semibold("Normal keys", 11.5, chip_text(normal)))
            .class(chip(normal))
            .padding([7, 12])
            .on_press(Message::SetLayer(None))
            .into(),
    );
    for layer in app.layers() {
        let active = app.layer.as_deref() == Some(layer.id.as_str());
        let hold = if layer.trigger.is_empty() {
            "choose a key…".to_owned()
        } else {
            format!("hold {}", short(&key_name(&layer.trigger)))
        };
        chips.push(
            widget::button::custom(
                widget::row::with_capacity(2)
                    .spacing(6)
                    .align_y(Alignment::Center)
                    .push(txt_semibold(layer.name.clone(), 11.5, chip_text(active)))
                    .push(txt(hold, 11.5, muted())),
            )
            .class(chip(active))
            .padding([7, 12])
            .on_press(Message::SetLayer(Some(layer.id.clone())))
            .into(),
        );
    }
    chips.push(
        widget::button::custom(txt_semibold("+ New layer", 11.5, tint(0.93, 0.02)))
            .class(accent_button())
            .padding([7, 12])
            .on_press(Message::AddLayer)
            .into(),
    );
    widget::flex_row(chips)
        .row_spacing(8)
        .column_spacing(8)
        .into()
}

/// Stable widget id for the layer rename input so it can be focused
/// when rename mode is entered.
pub fn layer_rename_input_id() -> widget::Id {
    widget::Id::new("layer-rename-input")
}

/// The look of a key that is being held: the tester's pressed cap, so
/// the layer key on the deck and in the banner read as pressed down.
fn held_cap() -> (Background, Color, Color) {
    (
        vgradient(tint(0.5, 0.1), tint(0.42, 0.09)),
        accent(),
        oklch(0.99, 0.01, 152.0),
    )
}

/// The printed legend of a deck key, falling back to its name for keys
/// whose cap is blank (Space).
fn legend(code: &str) -> String {
    match model::key(code).map(|cap| cap.label) {
        Some(label) if !label.is_empty() => label.to_owned(),
        _ => key_name(code),
    }
}

/// A miniature held keycap for the layer bar, matching the deck.
fn held_keycap(label: String) -> Element<'static, Message> {
    let (bg, border, color) = held_cap();
    container(txt_semibold(label, 12.0, color))
        .padding([4, 10])
        .class(ctheme::Container::custom(move |_| container::Style {
            background: Some(bg),
            border: Border {
                color: border,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        }))
        .into()
}

/// The strip above the deck while a layer is shown: its name, the key
/// that holds it, and what to do next.
fn layer_bar(app: &App) -> Option<Element<'_, Message>> {
    let layer = app.active_layer()?;
    let action = |label: &'static str, message: Message| {
        widget::button::custom(txt(label, 12.5, oklch(0.9, 0.01, 152.0)))
            .class(ghost_button())
            .padding([7, 12])
            .on_press(message)
    };

    let content: Element<'_, Message> = if app.choosing_layer_key {
        widget::row::with_capacity(4)
            .spacing(14)
            .align_y(Alignment::Center)
            .push(eyebrow("Layer key"))
            .push(
                widget::column::with_capacity(2)
                    .spacing(3)
                    .push(txt_semibold(
                        format!("Which key should hold {}?", layer.name),
                        14.0,
                        fg(),
                    ))
                    .push(txt(
                        "Click it on the keyboard below, or press it on your keyboard. Escape cancels.",
                        12.5,
                        muted(),
                    )),
            )
            .push(crate::ui::hspace())
            .push(action("Cancel", Message::CancelLayerKey))
            .into()
    } else {
        let name: Element<'_, Message> = if let Some(text) = &app.rename_layer {
            widget::text_input("Layer name", text)
                .id(layer_rename_input_id())
                .on_input(Message::RenameLayerInput)
                .on_submit(|_| Message::RenameLayerCommit)
                .width(Length::Fixed(220.0))
                .into()
        } else {
            txt_semibold(layer.name.clone(), 15.0, fg()).into()
        };
        let key = key_name(&layer.trigger);
        let jobs = layer.keys.len();
        let hint = format!(
            "Click a key to choose what it does while {key} is held. Bright: the held key. Tinted: keys with a job. Dimmed keys keep working normally · {jobs} key{} with a job",
            if jobs == 1 { "" } else { "s" }
        );
        widget::column::with_capacity(2)
            .spacing(8)
            .push(
                widget::row::with_capacity(9)
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .push(eyebrow("Layer"))
                    .push(name)
                    .push(txt("while", 12.5, muted()))
                    .push(held_keycap(legend(&layer.trigger)))
                    .push(txt("is held", 12.5, muted()))
                    .push(action("Change key", Message::ChooseLayerKey))
                    .push(crate::ui::hspace())
                    .push(action(
                        if app.rename_layer.is_some() {
                            "Cancel"
                        } else {
                            "Rename"
                        },
                        Message::RenameLayerToggle,
                    ))
                    .push(
                        widget::button::custom(txt("Delete layer", 12.5, oklch(0.85, 0.06, 16.0)))
                            .class(ghost_button())
                            .padding([7, 12])
                            .on_press(Message::DeleteLayer),
                    ),
            )
            .push(txt(hint, 12.5, muted()))
            .into()
    };
    Some(panel(content).padding([12, 16]).width(Length::Fill).into())
}

/// A 1px horizontal separator.
pub fn rule(color: Color) -> Element<'static, Message> {
    container(widget::Space::new().width(Length::Fill).height(1.0))
        .class(ctheme::Container::custom(move |_| container::Style {
            background: Some(color.into()),
            ..container::Style::default()
        }))
        .into()
}

/// The keyboard area: the tester's notice and panels (in the tester),
/// the deck, and the empty state or mapping summary.
pub fn area(app: &App) -> Element<'_, Message> {
    let mut column = widget::column::with_capacity(4)
        .spacing(20)
        .padding(Padding {
            top: 26.0,
            right: 30.0,
            bottom: 20.0,
            left: 30.0,
        });

    if app.view == View::Tester {
        if let Some(notice) = tester::notice(app) {
            column = column.push(notice);
        }
        column = column.push(tester::panels(app));
    }
    if app.view == View::Keyboard
        && let Some(bar) = layer_bar(app)
    {
        column = column.push(bar);
    }

    let (deck_width, deck_height) = model::deck_size(app.deck());
    // The deck centers itself in the available width; only when the
    // window is narrower does it fall back to a horizontal scroll.
    // (Inside the scrollable, width limits are unbounded, so a plain
    // centering `Fill` container would collapse to the deck's width.)
    column = column.push(
        container(widget::responsive(move |size| {
            let deck = container(canvas(app))
                .width(Length::Fixed(deck_width))
                .height(Length::Fixed(deck_height));
            let framed = container(deck).padding(5);
            let element: Element<'_, Message> = if size.width >= deck_width + 10.0 {
                framed.width(Length::Fill).align_x(Alignment::Center).into()
            } else {
                widget::scrollable::horizontal(framed)
                    .width(Length::Fill)
                    .into()
            };
            element
        }))
        .width(Length::Fill)
        .height(Length::Fixed(deck_height + 10.0)),
    );

    if app.view == View::Keyboard && app.layer.is_none() && app.selected.is_none() {
        if app.maps().is_empty() {
            column = column.push(
                container(
                    widget::row::with_capacity(2)
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .push(txt("No mappings in this profile yet.", 13.0, muted()))
                        .push(txt_semibold(
                            "Click a key above to choose its new action.",
                            13.0,
                            tint(0.82, 0.13),
                        ))
                        .width(Length::Shrink),
                )
                .width(Length::Fill)
                .align_x(Alignment::Center),
            );
        } else {
            column = column.push(summary(app));
        }
    }

    column.into()
}

/// Right-aligned access to the remaps dialog (`.remaps-summary-access`).
fn summary(app: &App) -> Element<'_, Message> {
    let count = app.maps().len();
    container(
        widget::button::custom(txt(
            format!("View remaps · {count}"),
            13.0,
            crate::ui::theme::fg(),
        ))
        .class(
            ButtonStyle {
                hover_bg: Some(Background::Color(white(0.05))),
                text: crate::ui::theme::fg(),
                border: crate::ui::theme::border(),
                border_width: 1.0,
                radius: 8.0,
                ..ButtonStyle::default()
            }
            .class(),
        )
        .padding([10, 15])
        .on_press(Message::OpenRemaps),
    )
    .width(Length::Fill)
    .align_x(Alignment::End)
    .into()
}

/// Chip text color helper.
fn chip_text(active: bool) -> Color {
    if active {
        tint(0.97, 0.02)
    } else {
        oklch(0.72, 0.01, 152.0)
    }
}

/// The displayed deck of key caps, absolutely positioned like the export.
fn canvas(app: &App) -> Element<'_, Message> {
    let keys = app.deck();
    let (deck_width, deck_height) = model::deck_size(keys);
    let mut layers: Vec<Element<'_, Message>> = Vec::with_capacity(keys.len() + 1);
    layers.push(
        widget::Space::new()
            .width(Length::Fixed(deck_width))
            .height(Length::Fixed(deck_height))
            .into(),
    );

    for cap in keys {
        layers.push(
            container(key_button(app, cap))
                .padding(Padding {
                    top: cap.y * model::UNIT,
                    left: cap.gx + cap.x * model::UNIT,
                    right: 0.0,
                    bottom: 0.0,
                })
                .width(Length::Fixed(deck_width))
                .height(Length::Fixed(deck_height))
                .into(),
        );
    }

    stack(layers)
        .width(Length::Fixed(deck_width))
        .height(Length::Fixed(deck_height))
        .into()
}

/// One key cap with all its visual states.
#[allow(clippy::too_many_lines)]
fn key_button<'a>(app: &'a App, cap: &'static model::KeyCap) -> Element<'a, Message> {
    let layer = app.active_layer();
    let nav_active = layer.is_some();
    let is_trigger = layer.is_some_and(|layer| layer.trigger == cap.code);
    let layer_label = layer
        .and_then(|layer| layer.key(cap.code))
        .map(|job| short(&job.action));
    // Keys that cannot take a job in this layer: physical modifiers
    // and the keys holding other layers.
    let locked = nav_active
        && !is_trigger
        && layer_label.is_none()
        && (xremap::is_modifier_key(cap.code) || app.layer_held_by(cap.code).is_some());
    // In the normal view, a key that holds a layer shows it like a
    // hold action.
    let holds_layer = if nav_active {
        None
    } else {
        app.layer_held_by(cap.code)
    };
    let mapping = app.mapping(cap.code);
    let selected = app.selected == Some(cap.code);
    let pressed = app.highlights_key(cap.evdev);

    // Base cap colors, overridden per state exactly like the export.
    let mut bg = vgradient(oklch(0.325, 0.007, 152.0), oklch(0.275, 0.007, 152.0));
    let mut border = oklch(0.36, 0.007, 152.0);
    let mut border_width = 1.0;
    let mut color = oklch(0.9, 0.008, 152.0);
    let mut outline = None;

    if nav_active && layer_label.is_none() && !is_trigger {
        bg = vgradient(oklch(0.26, 0.005, 152.0), oklch(0.235, 0.005, 152.0));
        color = if locked {
            oklch(0.4, 0.005, 152.0)
        } else {
            oklch(0.5, 0.006, 152.0)
        };
        border = oklch(0.29, 0.005, 152.0);
    }
    // A key with a job is tinted a little more than a mapped key in the
    // normal view, so the layer reads as the keyboard transforming; the
    // held layer key is the brightest cap, drawn like a pressed key.
    if layer_label.is_some() {
        bg = vgradient(tint(0.37, 0.05), tint(0.31, 0.045));
        border = tint(0.5, 0.085);
    } else if is_trigger {
        (bg, border, color) = held_cap();
    } else if (mapping.is_some() || holds_layer.is_some()) && !nav_active {
        bg = vgradient(tint(0.34, 0.032), tint(0.285, 0.028));
        border = tint(0.46, 0.075);
    }
    if selected {
        border = accent();
        border_width = 1.5;
        outline = Some((3.0, tint(0.5, 0.09).scale_alpha(0.35)));
    }
    if pressed {
        bg = vgradient(tint(0.5, 0.1), tint(0.42, 0.09));
        color = oklch(0.99, 0.01, 152.0);
        border = accent();
        border_width = 1.0;
    }

    // Cap legend: original label, mapped action or layer job, and the
    // hold line. A job is shown like a mapping: the printed legend
    // small on top, the job below.
    let tap_short = mapping
        .filter(|_| !nav_active)
        .and_then(|m| m.tap.as_deref())
        .map(short);
    let (main, main_size, main_color, main_weight): (String, f32, Color, Weight) =
        if let Some(label) = layer_label {
            (
                label.to_owned(),
                if label.chars().count() > 4 {
                    10.0
                } else {
                    12.0
                },
                accent(),
                Weight::Semibold,
            )
        } else if is_trigger {
            (
                cap.label.to_owned(),
                if cap.label.len() > 2 { 9.5 } else { 13.0 },
                color,
                Weight::Semibold,
            )
        } else if let Some(tap) = tap_short {
            (
                tap.to_owned(),
                if tap.len() > 4 { 10.0 } else { 12.0 },
                if pressed {
                    oklch(0.99, 0.01, 152.0)
                } else {
                    accent()
                },
                Weight::Semibold,
            )
        } else {
            (
                cap.label.to_owned(),
                if cap.label.len() > 2 { 9.5 } else { 13.0 },
                color,
                Weight::Medium,
            )
        };

    let show_orig = tap_short.is_some() || layer_label.is_some();
    let hold_line = if holds_layer.is_some() {
        Some("layer key".to_owned())
    } else if is_trigger {
        Some("held".to_owned())
    } else {
        mapping.filter(|_| !nav_active).and_then(|m| {
            m.hold
                .as_deref()
                .map(|hold| format!("hold {}", short(hold)))
                .or_else(|| m.swap.then(|| "⇄ two-way".to_owned()))
        })
    };

    let mut labels = widget::column::with_capacity(3)
        .spacing(2)
        .align_x(Alignment::Center);
    if show_orig {
        labels = labels.push(txt(cap.label.to_owned(), 8.0, muted()));
    }
    labels = labels.push(txt(main, main_size, main_color).font(Font {
        weight: main_weight,
        ..Font::DEFAULT
    }));
    if let Some(hold) = hold_line {
        let hold_color = if is_trigger {
            tint(0.97, 0.02)
        } else {
            oklch(0.72, 0.13, 16.0)
        };
        labels = labels.push(txt_semibold(hold, 7.5, hold_color));
    }

    let centered = container(labels)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    // Shortcut badge in the top-right corner of the cap.
    let combos = if nav_active {
        0
    } else {
        app.combos_for(&key_name(cap.code)).len()
    };
    let content: Element<'_, Message> = if combos > 0 {
        let badge = if combos > 1 {
            format!("{combos}⌘")
        } else {
            "⌘".to_owned()
        };
        let badge = container(txt_semibold(
            badge,
            7.0,
            if pressed {
                oklch(0.99, 0.01, 152.0)
            } else {
                oklch(0.72, 0.1, 196.0)
            },
        ))
        .width(Length::Fill)
        .align_x(Alignment::End)
        .padding(Padding {
            top: 2.0,
            right: 3.0,
            bottom: 0.0,
            left: 0.0,
        });
        stack([centered.into(), badge.into()])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        centered.into()
    };

    widget::button::custom(content)
        .class(
            ButtonStyle {
                bg: Some(bg),
                border,
                border_width,
                radius: 7.0,
                outline,
                ..ButtonStyle::default()
            }
            .class(),
        )
        .padding(0)
        .width(Length::Fixed(cap.w * model::UNIT - 5.0))
        .height(Length::Fixed(cap.h * model::UNIT - 5.0))
        .on_press(Message::SelectKey(cap.code))
        .into()
}

//! The key editor: pick what a selected key should do.

use cosmic::iced::widget::stack;
use cosmic::iced::{Alignment, Background, Border, Length, Padding};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message, Mode};
use crate::ui::model::{ACTION_GROUPS, MODS, auto_group, key_name, short};
use crate::ui::theme::{
    ButtonStyle, accent, accent_filled, black, border, fg, flat_button, flat_tab, keycap, muted,
    oklch, outline_button, quiet, tint, white,
};
use crate::ui::{Cap, keycap_chip, legend, mono, txt, txt_semibold};

/// The sheet title: the key being edited and, in a layer, the key
/// held for it, drawn as the deck draws them.
pub fn key_editor_title(app: &App) -> Element<'_, Message> {
    let Some(selected) = app.selected else {
        return widget::Space::new().into();
    };
    let word = |text: &'static str| txt_semibold(text, 16.0, fg());
    let mut title = widget::row::with_capacity(5)
        .spacing(8)
        .align_y(Alignment::Center);
    title = match app.active_layer() {
        Some(layer) => title
            .push(word("While holding"))
            .push(keycap_chip(legend(&layer.trigger), Cap::Held))
            .push(word("make")),
        None => title.push(word("Make")),
    };
    title
        .push(keycap_chip(legend(selected), Cap::Selected))
        .push(word("act as…"))
        .into()
}

/// The key editor content, shown in the app's context drawer while a
/// key is selected.
#[allow(clippy::too_many_lines)]
pub fn key_editor(app: &App) -> Element<'_, Message> {
    let Some(selected) = app.selected else {
        return widget::Space::new().into();
    };
    let mapping = app.mapping(selected);
    // While a layer is shown, the editor sets the key's job in it.
    let layer = app.active_layer();
    let job = layer.and_then(|layer| layer.key(selected));

    let mut section = widget::column::with_capacity(9).spacing(14);

    // Current behavior summary (the sheet title names the key): what
    // the key produces now, as keycaps like the deck's.
    let note = |text: String| txt(text, 13.0, oklch(0.78, 0.01, 152.0));
    let mut summary = widget::row::with_capacity(6)
        .spacing(8)
        .align_y(Alignment::Center)
        .push(note("Now:".to_owned()));
    if let Some(layer) = layer {
        summary = match job {
            Some(job) => summary.push(keycap_chip(short(&job.action), Cap::Mapped)),
            None => summary
                .push(keycap_chip(legend(selected), Cap::Plain))
                .push(note("normal".to_owned())),
        };
        summary = summary.push(note(format!("· in {}", layer.name)));
    } else {
        summary = match mapping.and_then(|m| m.tap.as_deref()) {
            Some(tap) => summary.push(keycap_chip(short(tap), Cap::Mapped)),
            None => summary
                .push(keycap_chip(legend(selected), Cap::Plain))
                .push(note("default".to_owned())),
        };
        if let Some(hold) = mapping.and_then(|m| m.hold.as_deref()) {
            summary = summary
                .push(note("· When held:".to_owned()))
                .push(keycap_chip(short(hold), Cap::Mapped));
        } else if let Some(held) = app.layer_held_by(selected) {
            summary = summary.push(note(format!("· When held: the {} layer", held.name)));
        }
    }
    section = section.push(summary);

    // Search plus key recording, side by side in the wide sheet.
    section = section.push(
        widget::row::with_capacity(2)
            .spacing(20)
            .align_y(Alignment::End)
            .push(
                widget::column::with_capacity(2)
                    .spacing(8)
                    .width(Length::Fill)
                    .push(txt("Find a key or action", 13.0, fg()))
                    .push(
                        widget::text_input("Search Escape, volume, letters…", &app.query)
                            .on_input(Message::Query),
                    ),
            )
            .push(
                widget::button::custom(txt(
                    if app.capture {
                        "Cancel recording · Esc"
                    } else {
                        "Record a key"
                    },
                    14.0,
                    fg(),
                ))
                .class(if app.capture {
                    quiet(true)
                } else {
                    outline_button()
                })
                .padding([10, 15])
                .on_press(Message::SetCapture(!app.capture)),
            ),
    );

    // Flat category tabs over a shared baseline (`.editor-categories`).
    let query = app.query.trim().to_lowercase();
    let active_group = app.category.unwrap_or_else(|| auto_group(selected));
    let tabs: Vec<Element<'_, Message>> = ACTION_GROUPS
        .iter()
        .map(|(name, _)| {
            let active = query.is_empty() && *name == active_group;
            let label = if active {
                txt_semibold(*name, 13.0, fg())
            } else {
                txt(*name, 13.0, fg())
            };
            let underline = container(widget::Space::new().width(Length::Fill).height(2.0)).class(
                ctheme::Container::custom(move |_| container::Style {
                    background: active.then(|| fg().into()),
                    ..container::Style::default()
                }),
            );
            widget::button::custom(
                widget::column::with_capacity(2)
                    .spacing(7)
                    .push(label)
                    .push(underline),
            )
            .class(flat_tab())
            .padding([9, 2])
            .on_press(Message::SetCategory(name))
            .into()
        })
        .collect();
    section = section.push(
        widget::column::with_capacity(2)
            .push(widget::flex_row(tabs).row_spacing(2).column_spacing(14))
            .push(crate::ui::keyboard_view::rule(border())),
    );

    // Action grid.
    let mut actions: Vec<(&'static str, &'static str)> = Vec::new();
    if query.is_empty() {
        if let Some((name, items)) = ACTION_GROUPS.iter().find(|(name, _)| *name == active_group) {
            actions.extend(items.iter().map(|action| (*action, *name)));
        }
    } else {
        for (name, items) in ACTION_GROUPS {
            for action in *items {
                if action.to_lowercase().contains(&query) {
                    actions.push((action, name));
                }
            }
        }
    }

    if actions.is_empty() {
        section = section.push(txt(
            "No matching actions. Try a key name such as Escape, or record a key.",
            14.0,
            muted(),
        ));
    } else {
        let from_mods: Vec<&str> = MODS
            .iter()
            .zip(app.from_mods)
            .filter_map(|(name, on)| on.then_some(*name))
            .collect();
        let combos = app.combos_for(&key_name(selected));

        let items: Vec<Element<'_, Message>> = actions
            .into_iter()
            .map(|(action, group)| {
                let active = if layer.is_some() {
                    job.map(|job| job.action.as_str()) == Some(action)
                } else {
                    match app.mode {
                        Mode::Combo => combos.iter().any(|(_, _, rule)| {
                            rule.to.key == action
                                && rule
                                    .from
                                    .mods
                                    .iter()
                                    .map(String::as_str)
                                    .collect::<Vec<_>>()
                                    == from_mods
                        }),
                        Mode::Hold => mapping.and_then(|m| m.hold.as_deref()) == Some(action),
                        Mode::Tap => mapping.and_then(|m| m.tap.as_deref()) == Some(action),
                    }
                };
                let label = if group == "Punctuation" {
                    format!("{}  {action}", short(action))
                } else {
                    action.to_owned()
                };
                keycap_cell(label, active, Message::PickAction(action.to_owned()))
            })
            .collect();

        section = section
            .push(container(widget::flex_row(items).row_spacing(10).column_spacing(10)).padding(5));
    }

    // Context line.
    let hint = if app.capture {
        "Recording output — press a key, or Escape to cancel".to_owned()
    } else if let Some(layer) = layer {
        format!(
            "Choose what {} does while {} is held",
            key_name(selected),
            key_name(&layer.trigger)
        )
    } else {
        match app.mode {
            Mode::Combo => "Also listed under Shortcuts",
            Mode::Hold => "Runs when the key is held",
            Mode::Tap => "Choose an action for a normal press",
        }
        .to_owned()
    };
    section = section.push(txt(
        format!(
            "{hint} · Applies to {}. Changes save and apply automatically.",
            app.device_label(&app.device)
        ),
        13.0,
        oklch(0.78, 0.01, 152.0),
    ));

    // Advanced options (`.editor-disclosure`). A layer job is a plain
    // action: nothing to hold, combine, or swap.
    if layer.is_none() {
        section = section.push(
            container(
                widget::button::custom(
                    widget::row::with_capacity(2)
                        .spacing(9)
                        .align_y(Alignment::Center)
                        .push(txt(if app.advanced { "▾" } else { "▸" }, 11.0, fg()))
                        .push(txt(
                            if app.advanced {
                                "Hide advanced options"
                            } else {
                                "Advanced options"
                            },
                            14.0,
                            fg(),
                        )),
                )
                .class(flat_button())
                .padding([10, 4])
                .on_press(Message::ToggleAdvanced),
            )
            .width(Length::Shrink),
        );
        if app.advanced {
            section = section.push(advanced_area(app, selected));
        }
    }

    section.into()
}

/// The drawer footer: restore and done actions for the key editor.
pub fn key_editor_footer(app: &App) -> Element<'_, Message> {
    let restore = if app.layer.is_some() {
        "Back to normal in this layer"
    } else {
        "Restore original key"
    };
    widget::row::with_capacity(3)
        .spacing(12)
        .push(
            widget::button::custom(txt(restore, 14.0, fg()))
                .class(flat_button())
                .padding([10, 15])
                .on_press(Message::ClearKey),
        )
        .push(crate::ui::hspace())
        .push(
            widget::button::custom(
                txt_semibold("Done", 14.0, crate::ui::theme::bg())
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
            .class(accent_filled())
            .padding([10, 24])
            .width(Length::Fixed(104.0))
            .on_press(Message::ClosePanel),
        )
        .into()
}

/// One tactile output keycap in the action grid, drawn with a solid
/// ledge: the key's darker front face showing under the cap, the way
/// a keycap's side does.
fn keycap_cell(label: String, active: bool, message: Message) -> Element<'static, Message> {
    const WIDTH: f32 = 148.0;
    const KEY_HEIGHT: f32 = 52.0;
    /// Height of the front face below the cap.
    const DROP: f32 = 3.0;

    let side = container(widget::Space::new())
        .width(Length::Fixed(WIDTH))
        .height(Length::Fixed(KEY_HEIGHT + DROP))
        .class(ctheme::Container::custom(move |_| container::Style {
            background: Some(
                if active {
                    tint(0.36, 0.08)
                } else {
                    oklch(0.2, 0.007, 152.0)
                }
                .into(),
            ),
            border: Border {
                radius: 6.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }));

    let cap = widget::button::custom(
        mono(
            label,
            13.0,
            if active {
                oklch(0.98, 0.01, 152.0)
            } else {
                fg()
            },
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .class(keycap(active))
    .padding([4, 12])
    .width(Length::Fixed(WIDTH))
    .height(Length::Fixed(KEY_HEIGHT))
    .on_press(message);

    stack([side.into(), cap.into()])
        .width(Length::Fixed(WIDTH))
        .height(Length::Fixed(KEY_HEIGHT + DROP))
        .into()
}

/// Hold, combo, and swap controls under the advanced toggle.
#[allow(clippy::too_many_lines)]
fn advanced_area<'a>(app: &'a App, selected: &'static str) -> Element<'a, Message> {
    let mapping = app.mapping(selected);
    let mut area = widget::column::with_capacity(6).spacing(12);

    area = area.push(crate::ui::keyboard_view::rule(oklch(0.36, 0.007, 152.0)));
    area = area.push(txt(
        "Add a different action when you hold this key, or use it with Ctrl, Shift, Alt or Super.",
        14.0,
        oklch(0.78, 0.01, 152.0),
    ));

    // Mode selection.
    let mode_button = |label: &'static str, mode: Mode| {
        widget::button::custom(txt(label, 13.0, oklch(0.95, 0.01, 152.0)))
            .class(quiet(app.mode == mode))
            .padding([9, 14])
            .on_press(Message::SetMode(mode))
            .into()
    };
    area = area.push(
        widget::flex_row(vec![
            mode_button("Normal press", Mode::Tap),
            mode_button("When held", Mode::Hold),
            mode_button("With other keys", Mode::Combo),
        ])
        .row_spacing(8)
        .column_spacing(8),
    );

    // Combo builder, stacked to fit the drawer.
    if app.mode == Mode::Combo {
        let mod_chip = |label: &'static str, on: bool, message: Message| {
            widget::button::custom(txt_semibold(
                label,
                11.0,
                if on { tint(0.97, 0.02) } else { muted() },
            ))
            .class(
                ButtonStyle {
                    bg: Some(Background::Color(if on {
                        tint(0.33, 0.05)
                    } else {
                        white(0.03)
                    })),
                    border: if on { accent() } else { white(0.09) },
                    border_width: 1.0,
                    radius: 7.0,
                    ..ButtonStyle::default()
                }
                .class(),
            )
            .padding([5, 10])
            .on_press(message)
            .into()
        };

        let mod_row = |label: &'static str, states: [bool; 4], message: fn(usize) -> Message| {
            let mut chips: Vec<Element<'_, Message>> =
                vec![container(txt(label, 10.0, muted())).padding([7, 0]).into()];
            for (index, name) in MODS.iter().enumerate() {
                chips.push(mod_chip(name, states[index], message(index)));
            }
            widget::flex_row(chips).row_spacing(6).column_spacing(6)
        };

        let picked: Vec<&str> = MODS
            .iter()
            .zip(app.from_mods)
            .filter_map(|(name, on)| on.then_some(*name))
            .collect();
        let combo_from = if picked.is_empty() {
            "pick a modifier".to_owned()
        } else {
            format!("{} + {}", picked.join(" + "), key_name(selected))
        };

        let mut builder = widget::column::with_capacity(5)
            .spacing(10)
            .push(mod_row("WITH", app.from_mods, Message::ToggleFromMod))
            .push(
                widget::row::with_capacity(2)
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .push(txt_semibold(combo_from, 12.0, oklch(0.9, 0.01, 152.0)))
                    .push(txt("→", 14.0, accent())),
            )
            .push(mod_row("SEND", app.to_mods, Message::ToggleToMod));
        if picked.is_empty() {
            builder = builder.push(txt(
                "Select at least one modifier for the input side.",
                11.0,
                oklch(0.72, 0.08, 16.0),
            ));
        }

        area = area.push(
            container(builder)
                .padding(Padding {
                    top: 9.0,
                    right: 12.0,
                    bottom: 9.0,
                    left: 12.0,
                })
                .width(Length::Fill)
                .class(ctheme::Container::custom(|_| container::Style {
                    background: Some(black(0.22).into()),
                    border: Border {
                        color: white(0.07),
                        width: 1.0,
                        radius: 10.0.into(),
                    },
                    ..container::Style::default()
                })),
        );
    }

    // Existing combos on this key.
    let combos = app.combos_for(&key_name(selected));
    if !combos.is_empty() {
        area = area.push(txt(
            format!(
                "{} combination{} on this key",
                combos.len(),
                if combos.len() == 1 { "" } else { "s" }
            ),
            14.0,
            oklch(0.78, 0.01, 152.0),
        ));
        for (group, rule_index, rule) in combos {
            let label = format!("{}+{}", rule.from.mods.join("+"), rule.from.key);
            let out = if rule.to.mods.is_empty() {
                rule.to.key.clone()
            } else {
                format!("{}+{}", rule.to.mods.join("+"), rule.to.key)
            };
            area = area.push(
                widget::flex_row(vec![
                    container(txt(format!("{label} → {out}"), 13.0, fg()))
                        .padding([8, 0])
                        .into(),
                    widget::button::custom(txt("Remove shortcut", 13.0, oklch(0.95, 0.01, 152.0)))
                        .class(quiet(false))
                        .padding([8, 12])
                        .on_press(Message::RemoveCombo {
                            group,
                            rule: rule_index,
                        })
                        .into(),
                ])
                .row_spacing(8)
                .column_spacing(12),
            );
        }
    }

    // Two-way swap.
    let tap = mapping.and_then(|m| m.tap.as_deref());
    let swap_on = mapping.is_some_and(|m| m.swap);
    let swap_sub = tap.map_or_else(
        || "set a tap action first".to_owned(),
        |tap| format!("{} → {}", short(tap), key_name(selected)),
    );
    let mut swap = widget::button::custom(txt(
        format!("Two-way swap · {swap_sub}"),
        13.0,
        oklch(0.95, 0.01, 152.0),
    ))
    .class(quiet(swap_on))
    .padding([9, 14]);
    if tap.is_some() {
        swap = swap.on_press(Message::ToggleSwap);
    }
    area = area.push(container(swap).width(Length::Shrink));

    area.into()
}

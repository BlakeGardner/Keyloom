//! The shortcuts view: rule groups and the bottom rule editor.

use cosmic::iced::{Alignment, Background, Border, Length, Padding};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, EditRule, Message, Side};
use crate::ui::theme::{
    ButtonStyle, accent, accent_button, ghost_button, muted, oklch, quiet, white,
};
use crate::ui::{chord_pills, eyebrow, txt, txt_semibold};

/// The shortcuts tab content.
pub fn view(app: &App) -> Element<'_, Message> {
    let groups = app.groups();
    let mut column = widget::column::with_capacity(groups.len() + 2).spacing(12);

    // Heading row.
    let note = if groups.is_empty() {
        String::new()
    } else {
        "Rules match top to bottom — the first group that matches wins.".to_owned()
    };
    column = column.push(
        widget::row::with_capacity(3)
            .align_y(Alignment::End)
            .push(
                widget::column::with_capacity(2)
                    .spacing(5)
                    .push(txt_semibold("Shortcuts", 19.0, oklch(0.95, 0.01, 152.0)))
                    .push(txt(note, 12.5, muted())),
            )
            .push(crate::ui::hspace())
            .push(new_group_button()),
    );

    if groups.is_empty() {
        column = column.push(
            container(
                widget::column::with_capacity(3)
                    .spacing(12)
                    .align_x(Alignment::Center)
                    .push(txt_semibold(
                        "No shortcut rules in this profile",
                        15.0,
                        oklch(0.9, 0.01, 152.0),
                    ))
                    .push(
                        txt(
                            "Shortcut rules turn one combination into another, optionally only inside a chosen application.",
                            12.5,
                            muted(),
                        )
                        .align_x(Alignment::Center)
                        .width(Length::Fixed(400.0)),
                    )
                    .push(new_group_button()),
            )
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .padding(Padding {
                top: 64.0,
                right: 0.0,
                bottom: 56.0,
                left: 0.0,
            }),
        );
    }

    for (gi, group) in groups.iter().enumerate() {
        // Group heading.
        let mut heading = widget::row::with_capacity(6)
            .spacing(10)
            .align_y(Alignment::Center)
            .push(txt_semibold(
                group.name.clone(),
                13.5,
                oklch(0.95, 0.01, 152.0),
            ));

        let scoped = !group.apps.is_empty();
        heading = heading.push(
            container(txt_semibold(
                group.scope_label(),
                10.5,
                if scoped {
                    oklch(0.92, 0.03, 196.0)
                } else {
                    muted()
                },
            ))
            .padding([4, 9])
            .class(ctheme::Container::custom(move |_| container::Style {
                background: Some(if scoped {
                    oklch(0.3, 0.05, 196.0).into()
                } else {
                    white(0.05).into()
                }),
                border: Border {
                    color: if scoped {
                        oklch(0.44, 0.07, 196.0)
                    } else {
                        white(0.08)
                    },
                    width: 1.0,
                    radius: 7.0.into(),
                },
                ..container::Style::default()
            })),
        );
        if group.any_mod {
            heading = heading.push(
                container(txt_semibold("Any modifier", 10.5, oklch(0.86, 0.08, 16.0)))
                    .padding([4, 9])
                    .class(ctheme::Container::custom(|_| container::Style {
                        background: Some(oklch(0.27, 0.03, 16.0).into()),
                        border: Border {
                            color: oklch(0.6, 0.1, 16.0),
                            width: 1.0,
                            radius: 7.0.into(),
                        },
                        ..container::Style::default()
                    })),
            );
        }
        heading = heading.push(crate::ui::hspace());
        heading = heading.push(txt(
            format!(
                "{} rule{}",
                group.rules.len(),
                if group.rules.len() == 1 { "" } else { "s" }
            ),
            11.0,
            muted(),
        ));
        heading = heading.push(
            widget::button::custom(txt(
                if group.enabled { "Enabled" } else { "Paused" },
                13.0,
                oklch(0.95, 0.01, 152.0),
            ))
            .class(quiet(group.enabled))
            .padding([8, 12])
            .on_press(Message::ToggleGroup(gi)),
        );

        // Two-column rule grid.
        let editing = app.edit_rule;
        let mut cells: Vec<Element<'_, Message>> = Vec::with_capacity(group.rules.len() + 1);
        for (ri, rule) in group.rules.iter().enumerate() {
            let active = editing
                == Some(EditRule {
                    group: gi,
                    rule: Some(ri),
                });
            let dim = if group.enabled { 1.0 } else { 0.45 };

            let mut content = widget::row::with_capacity(5)
                .spacing(9)
                .align_y(Alignment::Center)
                .push(chord_pills(&rule.from))
                .push(txt("→", 12.0, accent().scale_alpha(dim)))
                .push(chord_pills(&rule.to))
                .push(crate::ui::hspace());
            if !rule.note.is_empty() {
                content = content.push(txt(rule.note.clone(), 11.0, muted().scale_alpha(dim)));
            }

            cells.push(
                widget::button::custom(content)
                    .class(
                        ButtonStyle {
                            bg: Some(Background::Color(if active {
                                oklch(0.3, 0.04, 152.0)
                            } else {
                                white(0.028 * dim)
                            })),
                            border: if active { accent() } else { white(0.07 * dim) },
                            border_width: 1.0,
                            radius: 9.0,
                            ..ButtonStyle::default()
                        }
                        .class(),
                    )
                    .padding([8, 11])
                    .width(Length::Fill)
                    .on_press(Message::EditRule {
                        group: gi,
                        rule: Some(ri),
                    })
                    .into(),
            );
        }
        cells.push(
            widget::button::custom(txt_semibold("+ Add shortcut", 11.5, muted()))
                .class(
                    ButtonStyle {
                        hover_bg: Some(Background::Color(white(0.04))),
                        text: muted(),
                        hover_text: Some(oklch(0.85, 0.01, 152.0)),
                        border: white(0.14),
                        border_width: 1.0,
                        radius: 9.0,
                        ..ButtonStyle::default()
                    }
                    .class(),
                )
                .padding([8, 11])
                .width(Length::Fill)
                .on_press(Message::EditRule {
                    group: gi,
                    rule: None,
                })
                .into(),
        );

        let mut grid = widget::column::with_capacity(cells.len().div_ceil(2)).spacing(8);
        let mut cells = cells.into_iter();
        while let Some(first) = cells.next() {
            let mut pair = widget::row::with_capacity(2).spacing(8).push(first);
            if let Some(second) = cells.next() {
                pair = pair.push(second);
            } else {
                pair = pair.push(widget::Space::new().width(Length::Fill));
            }
            grid = grid.push(pair);
        }

        column = column.push(
            container(
                widget::column::with_capacity(2)
                    .spacing(12)
                    .push(heading)
                    .push(grid),
            )
            .width(Length::Fill)
            .padding(Padding {
                top: 14.0,
                right: 16.0,
                bottom: 15.0,
                left: 16.0,
            })
            .class(ctheme::Container::custom(|_| container::Style {
                background: Some(oklch(0.235, 0.008, 152.0).into()),
                border: Border {
                    color: white(0.06),
                    width: 1.0,
                    radius: 13.0.into(),
                },
                ..container::Style::default()
            })),
        );
    }

    container(column)
        .width(Length::Fill)
        .padding(Padding {
            top: 24.0,
            right: 30.0,
            bottom: 28.0,
            left: 30.0,
        })
        .into()
}

fn new_group_button() -> Element<'static, Message> {
    widget::button::custom(txt_semibold("+ New group", 12.0, oklch(0.93, 0.02, 152.0)))
        .class(accent_button())
        .padding([8, 14])
        .on_press(Message::AddGroup)
        .into()
}

/// The bottom editor for one shortcut rule.
#[allow(clippy::too_many_lines)]
pub fn editor(app: &App) -> Element<'_, Message> {
    let Some(edit) = app.edit_rule else {
        return widget::Space::new().into();
    };
    let Some(group) = app.groups().get(edit.group) else {
        return widget::Space::new().into();
    };
    let rule = edit.rule.and_then(|index| group.rules.get(index));

    // Left: rule context and delete.
    let left = widget::column::with_capacity(4)
        .spacing(10)
        .push(eyebrow(if edit.rule.is_some() {
            "Edit shortcut"
        } else {
            "New shortcut"
        }))
        .push(txt_semibold(
            group.name.clone(),
            15.0,
            oklch(0.95, 0.01, 152.0),
        ))
        .push(txt(group.scope_label(), 11.5, muted()))
        .push(
            widget::button::custom(txt("Delete shortcut", 12.0, oklch(0.85, 0.06, 16.0)))
                .class(ghost_button())
                .padding([7, 12])
                .on_press(Message::DeleteRule),
        );

    // Middle: the two recorded chords.
    let record_button = |side: Side| {
        let recording = app.recording == Some(side);
        widget::button::custom(txt_semibold(
            if recording { "Listening…" } else { "Record" },
            10.5,
            if recording {
                oklch(0.97, 0.02, 152.0)
            } else {
                oklch(0.85, 0.01, 152.0)
            },
        ))
        .class(
            ButtonStyle {
                bg: Some(Background::Color(if recording {
                    oklch(0.34, 0.06, 152.0)
                } else {
                    white(0.05)
                })),
                border: if recording { accent() } else { white(0.12) },
                border_width: 1.0,
                radius: 7.0,
                ..ButtonStyle::default()
            }
            .class(),
        )
        .padding([4, 10])
        .on_press(Message::SetRecording(if recording {
            None
        } else {
            Some(side)
        }))
    };

    let chord_box = |side: Side, chord: Option<&crate::ui::model::Chord>| {
        let recording = app.recording == Some(side);
        container(match chord {
            Some(chord) => chord_pills(chord),
            None => chord_pills(&crate::ui::model::Chord::default()),
        })
        .width(Length::Fill)
        .padding([9, 11])
        .class(ctheme::Container::custom(move |_| container::Style {
            background: Some(if recording {
                oklch(0.28, 0.035, 152.0).into()
            } else {
                crate::ui::theme::black(0.2).into()
            }),
            border: Border {
                color: if recording { accent() } else { white(0.09) },
                width: 1.0,
                radius: 10.0.into(),
            },
            ..container::Style::default()
        }))
    };

    let side_column = |label: &'static str, side: Side| {
        widget::column::with_capacity(2)
            .spacing(7)
            .width(Length::Fill)
            .push(
                widget::row::with_capacity(2)
                    .align_y(Alignment::Center)
                    .push(eyebrow(label))
                    .push(crate::ui::hspace())
                    .push(record_button(side)),
            )
            .push(chord_box(
                side,
                rule.map(|rule| match side {
                    Side::From => &rule.from,
                    Side::To => &rule.to,
                }),
            ))
    };

    let hint = if app.recording.is_some() {
        "Hold the modifiers, then press a key. Escape cancels.".to_owned()
    } else if rule.is_some_and(|rule| !rule.from.key.is_empty() && !rule.to.key.is_empty()) {
        "Both sides recorded. This shortcut is ready.".to_owned()
    } else {
        "Record both sides to complete this shortcut.".to_owned()
    };

    let middle = widget::column::with_capacity(2)
        .spacing(10)
        .width(Length::Fill)
        .push(
            widget::row::with_capacity(3)
                .spacing(14)
                .align_y(Alignment::Center)
                .push(side_column("When I press", Side::From))
                .push(txt("→", 18.0, accent()))
                .push(side_column("Send instead", Side::To)),
        )
        .push(txt(hint, 11.0, muted()));

    // Right: group behaviour and done.
    let any_mod = group.any_mod;
    let right = widget::column::with_capacity(4)
        .spacing(10)
        .push(eyebrow("Group behaviour"))
        .push(
            widget::button::custom(
                widget::row::with_capacity(2)
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .push(txt_semibold(
                        "Ignore modifiers",
                        11.5,
                        if any_mod {
                            oklch(0.96, 0.02, 152.0)
                        } else {
                            oklch(0.8, 0.01, 152.0)
                        },
                    ))
                    .push(txt("any held", 10.0, muted())),
            )
            .class(
                ButtonStyle {
                    bg: Some(Background::Color(if any_mod {
                        oklch(0.31, 0.045, 152.0)
                    } else {
                        white(0.03)
                    })),
                    border: if any_mod { accent() } else { white(0.08) },
                    border_width: 1.0,
                    radius: 9.0,
                    ..ButtonStyle::default()
                }
                .class(),
            )
            .padding([8, 11])
            .on_press(Message::ToggleAnyMod),
        )
        .push(
            txt(
                "Matches the shortcut even while Shift, Ctrl, Alt or Super is held.",
                11.0,
                muted(),
            )
            .width(Length::Fill),
        )
        .push({
            let mut done = widget::button::custom(txt_semibold(
                "Done",
                12.0,
                oklch(0.88, 0.01, 152.0),
            ))
            .class(ghost_button())
            .padding([7, 12]);
            let complete =
                rule.is_some_and(|rule| !rule.from.key.is_empty() && !rule.to.key.is_empty());
            if complete {
                done = done.on_press(Message::CloseEdit);
            }
            done
        });

    let vertical_rule = || {
        container(widget::Space::new().width(1.0).height(Length::Fill)).class(
            ctheme::Container::custom(|_| container::Style {
                background: Some(white(0.06).into()),
                ..container::Style::default()
            }),
        )
    };

    container(
        widget::row::with_capacity(5)
            .push(
                container(left)
                    .width(Length::Fixed(236.0))
                    .padding([20, 22]),
            )
            .push(vertical_rule())
            .push(
                container(middle)
                    .width(Length::Fill)
                    .padding(Padding {
                        top: 18.0,
                        right: 22.0,
                        bottom: 20.0,
                        left: 22.0,
                    }),
            )
            .push(vertical_rule())
            .push(
                container(right)
                    .width(Length::Fixed(212.0))
                    .padding([20, 22]),
            ),
    )
    .width(Length::Fill)
    .class(ctheme::Container::custom(|_| container::Style {
        background: Some(oklch(0.185, 0.007, 152.0).into()),
        border: Border {
            color: white(0.08),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }))
    .into()
}

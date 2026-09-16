//! The shortcuts view: rule groups in sections per application scope,
//! plus the rule editor shown in the app's context drawer.

use cosmic::iced::{Alignment, Background, Border, Length, Padding};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, EditRule, Message, Popover, Side};
use crate::ui::theme::{
    ButtonStyle, accent, accent_button, accent_filled, ghost_button, muted, oklch, quiet, tint,
    white,
};
use crate::ui::{chord_pills, eyebrow, txt, txt_semibold};

/// The shortcuts tab content.
#[allow(clippy::too_many_lines)]
pub fn view(app: &App) -> Element<'_, Message> {
    let groups = app.groups();
    let mut column = widget::column::with_capacity(groups.len() + 2).spacing(12);

    // Heading row.
    let note = if groups.is_empty() {
        String::new()
    } else {
        "Rules match top to bottom — the first group that matches wins, and a group limited to an application comes before the rest.".to_owned()
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

    // Groups in sections per application scope, in the order their
    // rules apply: each scope of the profile, then every application.
    let mut sections: Vec<(String, Vec<usize>)> = Vec::with_capacity(app.app_scopes().len() + 1);
    for scope in app.app_scopes() {
        let members: Vec<usize> = groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.scope == scope.id)
            .map(|(gi, _)| gi)
            .collect();
        if !members.is_empty() {
            sections.push((format!("In {}", scope.name), members));
        }
    }
    let general: Vec<usize> = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| !app.app_scopes().iter().any(|scope| scope.id == group.scope))
        .map(|(gi, _)| gi)
        .collect();
    if !general.is_empty() {
        sections.push(("In all applications".to_owned(), general));
    }
    let labelled = sections.len() > 1
        || sections
            .iter()
            .any(|(label, _)| label != "In all applications");

    for (label, members) in sections {
        if labelled {
            column = column.push(container(eyebrow(&label)).padding(Padding {
                top: 6.0,
                right: 4.0,
                bottom: 0.0,
                left: 4.0,
            }));
        }
        for gi in members {
            column = column.push(group_card(app, gi));
        }
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

/// One group: heading with its application scope chip, the rule
/// grid, and the way to add a rule.
fn group_card(app: &App, gi: usize) -> Element<'_, Message> {
    let group = &app.groups()[gi];
    // Group heading.
    let mut heading = widget::row::with_capacity(6)
        .spacing(10)
        .align_y(Alignment::Center)
        .push(txt_semibold(
            group.name.clone(),
            13.5,
            oklch(0.95, 0.01, 152.0),
        ));

    // The scope chip opens the chooser: every application, one of
    // the profile's application scopes, or a new one.
    let scoped = app.app_scopes().iter().any(|scope| scope.id == group.scope);
    let chip = widget::button::custom(
        widget::row::with_capacity(2)
            .spacing(6)
            .align_y(Alignment::Center)
            .push(txt_semibold(
                app.app_scope_name(&group.scope),
                10.5,
                if scoped {
                    oklch(0.92, 0.03, 196.0)
                } else {
                    muted()
                },
            ))
            .push(txt("▾", 8.0, muted())),
    )
    .class(
        ButtonStyle {
            bg: Some(Background::Color(if scoped {
                oklch(0.3, 0.05, 196.0)
            } else {
                white(0.05)
            })),
            hover_bg: Some(Background::Color(if scoped {
                oklch(0.36, 0.06, 196.0)
            } else {
                white(0.09)
            })),
            border: if scoped {
                oklch(0.44, 0.07, 196.0)
            } else {
                white(0.08)
            },
            border_width: 1.0,
            radius: 7.0,
            ..ButtonStyle::default()
        }
        .class(),
    )
    .padding([4, 9])
    .on_press(Message::TogglePopover(Popover::GroupScope(gi)));
    let chip: Element<'_, Message> = if app.popover == Some(Popover::GroupScope(gi)) {
        widget::popover(chip)
            .popup(crate::ui::overlays::group_scope_popup(app, gi))
            .position(widget::popover::Position::Bottom)
            .on_close(Message::CloseOverlays)
            .into()
    } else {
        chip.into()
    };
    heading = heading.push(chip);
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
                            tint(0.3, 0.04)
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
    }))
    .into()
}

fn new_group_button() -> Element<'static, Message> {
    widget::button::custom(txt_semibold("+ New group", 12.0, tint(0.93, 0.02)))
        .class(accent_button())
        .padding([8, 14])
        .on_press(Message::AddGroup)
        .into()
}

/// The rule editor content, shown in the app's context drawer while a
/// shortcut is being edited.
pub fn rule_editor(app: &App) -> Element<'_, Message> {
    let Some(edit) = app.edit_rule else {
        return widget::Space::new().into();
    };
    let Some(group) = app.groups().get(edit.group) else {
        return widget::Space::new().into();
    };
    let rule = edit.rule.and_then(|index| group.rules.get(index));

    let record_button = |side: Side| {
        let recording = app.recording == Some(side);
        widget::button::custom(txt_semibold(
            if recording { "Listening…" } else { "Record" },
            10.5,
            if recording {
                tint(0.97, 0.02)
            } else {
                oklch(0.85, 0.01, 152.0)
            },
        ))
        .class(
            ButtonStyle {
                bg: Some(Background::Color(if recording {
                    tint(0.34, 0.06)
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
                tint(0.28, 0.035).into()
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

    let side_section = |label: &'static str, side: Side| {
        widget::column::with_capacity(2)
            .spacing(7)
            .width(Length::Fill)
            .push(
                widget::row::with_capacity(3)
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
        "Hold the modifiers, then press a key. Escape cancels."
    } else if rule.is_some_and(crate::ui::model::Rule::is_complete) {
        "Both sides recorded. This shortcut is ready."
    } else {
        "Record both sides to complete this shortcut."
    };

    let any_mod = group.any_mod;

    // Wide sheet layout, mirroring the design's original bottom panel:
    // context · when-I-press → send-instead · group behaviour.
    let context = widget::column::with_capacity(2)
        .spacing(4)
        .width(Length::Fixed(190.0))
        .push(txt_semibold(
            group.name.clone(),
            15.0,
            oklch(0.95, 0.01, 152.0),
        ))
        .push(txt(app.app_scope_name(&group.scope), 11.5, muted()));

    let behaviour = widget::column::with_capacity(3)
        .spacing(10)
        .width(Length::Fixed(230.0))
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
                            tint(0.96, 0.02)
                        } else {
                            oklch(0.8, 0.01, 152.0)
                        },
                    ))
                    .push(txt("any held", 10.0, muted())),
            )
            .class(
                ButtonStyle {
                    bg: Some(Background::Color(if any_mod {
                        tint(0.31, 0.045)
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
        .push(txt(
            "Matches the shortcut even while Shift, Ctrl, Alt or Super is held.",
            11.0,
            muted(),
        ));

    let chords = widget::column::with_capacity(2)
        .spacing(10)
        .width(Length::Fill)
        .push(
            widget::row::with_capacity(3)
                .spacing(14)
                .align_y(Alignment::Center)
                .push(side_section("When I press", Side::From))
                .push(txt("→", 18.0, accent()))
                .push(side_section("Send instead", Side::To)),
        )
        .push(txt(hint, 11.0, muted()));

    widget::row::with_capacity(3)
        .spacing(24)
        .push(context)
        .push(chords)
        .push(behaviour)
        .into()
}

/// The drawer footer: delete and done actions for the rule editor.
pub fn rule_editor_footer(app: &App) -> Element<'_, Message> {
    let complete = app
        .edit_rule
        .and_then(|edit| {
            app.groups()
                .get(edit.group)
                .and_then(|group| edit.rule.and_then(|index| group.rules.get(index)))
        })
        .is_some_and(crate::ui::model::Rule::is_complete);

    let mut done = widget::button::custom(
        txt_semibold("Done", 14.0, crate::ui::theme::bg())
            .align_x(Alignment::Center)
            .width(Length::Fill),
    )
    .class(accent_filled())
    .padding([10, 24])
    .width(Length::Fixed(104.0));
    if complete {
        done = done.on_press(Message::CloseEdit);
    }

    widget::row::with_capacity(3)
        .spacing(12)
        .push(
            widget::button::custom(txt("Delete shortcut", 12.0, oklch(0.85, 0.06, 16.0)))
                .class(ghost_button())
                .padding([10, 12])
                .on_press(Message::DeleteRule),
        )
        .push(crate::ui::hspace())
        .push(done)
        .into()
}

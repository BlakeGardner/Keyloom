//! The tester view's three panels: last key, held modifiers, output,
//! and the notice shown when remapping is holding a keyboard.

use cosmic::iced::{Alignment, Border, Length, Shadow, Vector};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message};
use crate::ui::model::{self, key_name};
use crate::ui::theme::{accent, muted, oklch, oklcha, vgradient};
use crate::ui::{eyebrow, mono, panel, txt, txt_semibold};

/// Why the selected keyboard is silent.
///
/// A remapper grabs the keyboards it takes over, so their keys reach it
/// alone and the tester would otherwise sit there looking broken. The
/// way through is the header's remapping chip, which releases the
/// keyboard until it is pressed again. Nothing is shown once remapping
/// is off: no keyboard is held, and the chip already says so.
pub fn notice(app: &App) -> Option<Element<'_, Message>> {
    let held = app.grabbed_selection()?;
    let title = format!("{} is being remapped", held.name);
    let detail = "pause remapping in the header to see its real key presses";

    Some(
        container(
            widget::row::with_capacity(2)
                .spacing(10)
                .align_y(Alignment::Center)
                .push(txt_semibold(title, 12.5, oklch(0.95, 0.02, 152.0)))
                .push(txt(detail, 12.0, muted())),
        )
        .width(Length::Fill)
        .padding([10, 16])
        .class(ctheme::Container::custom(|_| container::Style {
            background: Some(oklch(0.25, 0.02, 152.0).into()),
            border: Border {
                color: oklch(0.45, 0.07, 152.0),
                width: 1.0,
                radius: 11.0.into(),
            },
            ..container::Style::default()
        }))
        .into(),
    )
}

/// The row of tester panels above the keyboard.
pub fn panels(app: &App) -> Element<'_, Message> {
    widget::row::with_capacity(3)
        .spacing(14)
        .push(key_panel(app).width(Length::Fill))
        .push(modifier_panel(app).width(Length::Fixed(250.0)))
        .push(output_panel(app).width(Length::Fixed(210.0)))
        .into()
}

/// Big key cap plus the name/code of the last observed key.
fn key_panel(app: &App) -> crate::ui::Panel<'_> {
    let last = app.last.as_ref();

    let cap_label = last.map_or("—".to_owned(), |last| {
        let label = model::key(last.code).map_or("?", |cap| cap.label);
        if label.is_empty() {
            "Space".to_owned()
        } else {
            label.to_owned()
        }
    });
    let has_last = last.is_some();
    let cap = container(
        txt_semibold(
            cap_label,
            if has_last { 18.0 } else { 22.0 },
            if has_last {
                oklch(0.99, 0.01, 152.0)
            } else {
                muted()
            },
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fixed(64.0))
    .height(Length::Fixed(64.0))
    .class(ctheme::Container::custom(move |_| container::Style {
        background: Some(if has_last {
            vgradient(oklch(0.48, 0.1, 152.0), oklch(0.4, 0.09, 152.0))
        } else {
            oklch(0.27, 0.006, 152.0).into()
        }),
        border: Border {
            color: if has_last {
                accent()
            } else {
                oklch(0.4, 0.006, 152.0)
            },
            width: 1.0,
            radius: 11.0.into(),
        },
        shadow: if has_last {
            Shadow {
                color: oklcha(0.7, 0.14, 152.0, 0.7),
                offset: Vector::ZERO,
                blur_radius: 26.0,
            }
        } else {
            Shadow::default()
        },
        ..container::Style::default()
    }));

    let title = last.map_or_else(|| "Press any key".to_owned(), |last| key_name(last.code));
    let code = last.map_or_else(
        || {
            if app.grabbed_selection().is_some() {
                "This keyboard is being remapped".to_owned()
            } else {
                format!("Listening to {}", app.device_label(&app.device))
            }
        },
        |last| last.code.to_owned(),
    );
    let device = last.map_or_else(
        || "The tester works before you make a single mapping.".to_owned(),
        |last| last.device.clone(),
    );

    panel(
        widget::row::with_capacity(2)
            .spacing(18)
            .align_y(Alignment::Center)
            .push(cap)
            .push(
                widget::column::with_capacity(3)
                    .spacing(5)
                    .push(txt_semibold(title, 16.0, oklch(0.95, 0.01, 152.0)))
                    .push(mono(code, 10.5, muted()))
                    .push(txt(device, 11.5, muted())),
            ),
    )
    .padding([16, 20])
}

/// Live view of which modifiers are held.
fn modifier_panel(app: &App) -> crate::ui::Panel<'_> {
    let [ctrl, shift, alt, sup] = app.held_mods();
    let chips = [
        ("Shift", shift),
        ("Control", ctrl),
        ("Alt", alt),
        ("Super", sup),
    ];

    let chips: Vec<Element<'_, Message>> = chips
        .into_iter()
        .map(|(label, held)| {
            container(txt_semibold(
                label,
                11.5,
                if held {
                    oklch(0.97, 0.02, 152.0)
                } else {
                    muted()
                },
            ))
            .padding([5, 11])
            .class(ctheme::Container::custom(move |_| container::Style {
                background: Some(if held {
                    oklch(0.36, 0.06, 152.0).into()
                } else {
                    crate::ui::theme::white(0.04).into()
                }),
                border: Border {
                    color: if held {
                        accent()
                    } else {
                        crate::ui::theme::white(0.08)
                    },
                    width: 1.0,
                    radius: 7.0.into(),
                },
                ..container::Style::default()
            }))
            .into()
        })
        .collect();

    panel(
        widget::column::with_capacity(2)
            .spacing(9)
            .push(eyebrow("Modifiers"))
            .push(widget::flex_row(chips).row_spacing(6).column_spacing(6)),
    )
    .padding([14, 18])
}

/// What the last key becomes under the active profile.
fn output_panel(app: &App) -> crate::ui::Panel<'_> {
    let last = app.last.as_ref();
    let mapping = last.and_then(|last| app.mapping(last.code));
    let mapped = mapping.and_then(|mapping| mapping.tap.clone());

    let output = last.map_or_else(
        || "—".to_owned(),
        |last| mapped.clone().unwrap_or_else(|| key_name(last.code)),
    );
    let note = last.map_or_else(
        || "output appears here".to_owned(),
        |_| {
            if mapped.is_some() {
                format!("remapped in {}", app.profile_name())
            } else {
                "no mapping — passes through".to_owned()
            }
        },
    );

    panel(
        widget::column::with_capacity(3)
            .spacing(8)
            .push(eyebrow("Becomes"))
            .push(txt_semibold(
                output,
                19.0,
                if mapped.is_some() {
                    accent()
                } else {
                    oklch(0.9, 0.01, 152.0)
                },
            ))
            .push(txt(note, 11.0, muted())),
    )
    .padding([14, 18])
}

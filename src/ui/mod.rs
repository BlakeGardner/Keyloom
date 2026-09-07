//! The Keyloom interface, following the design export in `design/`.

pub mod editor;
pub mod header;
pub mod keyboard_view;
pub mod model;
pub mod overlays;
pub mod shortcuts;
pub mod tester;
pub mod theme;

use cosmic::iced::font::Weight;
use cosmic::iced::widget::stack;
use cosmic::iced::{Alignment, Border, Color, Font, Length, Shadow, Vector};
use cosmic::iced::core::text::LineHeight;
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message, View};
use theme::{black, oklch, surface, white};

/// Maximum width of the app shell (`.app-width` in the export).
const SHELL_MAX_WIDTH: f32 = 1220.0;

/// Text widget carrying cosmic's theme and renderer generics.
pub type Txt<'a> = widget::Text<'a, cosmic::Theme, cosmic::Renderer>;

/// Container widget carrying cosmic's theme generic.
pub type Panel<'a> = widget::Container<'a, Message, cosmic::Theme>;

/// A text fragment with explicit size and color, the workhorse of this
/// design's typography.
pub fn txt<'a>(content: impl Into<String>, size: f32, color: Color) -> Txt<'a> {
    widget::text(content.into())
        .size(size)
        .class(ctheme::Text::Color(color))
}

/// [`txt`] with a semibold weight.
pub fn txt_semibold<'a>(content: impl Into<String>, size: f32, color: Color) -> Txt<'a> {
    txt(content, size, color).font(Font {
        weight: Weight::Semibold,
        ..Font::DEFAULT
    })
}

/// [`txt`] in the monospace face used for key codes.
pub fn mono<'a>(content: impl Into<String>, size: f32, color: Color) -> Txt<'a> {
    txt(content, size, color).font(Font::MONOSPACE)
}

/// A horizontal filler, like `flex: 1`.
pub fn hspace() -> widget::Space {
    widget::Space::new().width(Length::Fill)
}

/// Small uppercase section label (`.eyebrow`).
pub fn eyebrow<'a>(content: &str) -> Txt<'a> {
    txt(content.to_uppercase(), 10.0, oklch(0.75, 0.01, 152.0)).font(Font {
        weight: Weight::Medium,
        ..Font::DEFAULT
    })
}

/// Card-like panel used by the tester and shortcut groups.
pub fn panel<'a>(content: impl Into<Element<'a, Message>>) -> Panel<'a> {
    container(content).class(ctheme::Container::custom(|_| container::Style {
        background: Some(oklch(0.235, 0.008, 152.0).into()),
        border: Border {
            color: white(0.07),
            width: 1.0,
            radius: 13.0.into(),
        },
        ..container::Style::default()
    }))
}

/// Floating popover/dialog chrome.
pub fn popover_panel<'a>(content: impl Into<Element<'a, Message>>) -> Panel<'a> {
    container(content)
        .padding(8)
        .class(ctheme::Container::custom(|_| container::Style {
            background: Some(oklch(0.26, 0.008, 152.0).into()),
            border: Border {
                color: white(0.12),
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: Shadow {
                color: black(0.7),
                offset: Vector::new(0.0, 24.0),
                blur_radius: 50.0,
            },
            ..container::Style::default()
        }))
}

/// The kinds of chord pills used across the shortcut views.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pill {
    /// "Any modifier" (dashed red in the design).
    Any,
    /// A plain modifier.
    Mod,
    /// Placeholder when nothing is recorded yet.
    Empty,
    /// The chord's key (input or output side).
    Key,
}

/// One chord pill (`pillStyle` in the export).
pub fn pill(label: &str, kind: Pill) -> Element<'static, Message> {
    let (bg, border, color) = match kind {
        Pill::Any => (
            Some(oklch(0.28, 0.03, 16.0)),
            oklch(0.6, 0.1, 16.0),
            oklch(0.86, 0.08, 16.0),
        ),
        Pill::Mod => (Some(white(0.05)), white(0.10), oklch(0.78, 0.01, 152.0)),
        Pill::Empty => (None, white(0.18), theme::muted()),
        Pill::Key => (
            Some(oklch(0.31, 0.04, 152.0)),
            oklch(0.44, 0.07, 152.0),
            oklch(0.95, 0.02, 152.0),
        ),
    };
    container(txt_semibold(label, 11.0, color).line_height(LineHeight::Absolute(12.0.into())))
        .padding([5, 8])
        .class(ctheme::Container::custom(move |_| container::Style {
            background: bg.map(Into::into),
            border: Border {
                color: border,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        }))
        .into()
}

/// Render a chord as a row of pills.
pub fn chord_pills(chord: &model::Chord) -> Element<'static, Message> {
    let mut row = widget::row::with_capacity(chord.mods.len() + 1)
        .spacing(4)
        .align_y(Alignment::Center);
    if chord.is_empty() {
        return row.push(pill("—", Pill::Empty)).into();
    }
    for modifier in &chord.mods {
        if modifier == "Any" {
            row = row.push(pill("Any modifier", Pill::Any));
        } else {
            row = row.push(pill(modifier, Pill::Mod));
        }
    }
    if !chord.key.is_empty() {
        row = row.push(pill(&chord.key, Pill::Key));
    }
    row.into()
}

/// Build the whole window content: the app shell plus overlay layers.
pub fn view(app: &App) -> Element<'_, Message> {
    let mut shell = widget::column::with_capacity(4);

    match app.view {
        View::Keyboard | View::Tester => {
            shell = shell.push(keyboard_view::device_toolbar(app));
            shell = shell.push(keyboard_view::area(app));
        }
        View::Shortcuts => {
            shell = shell.push(shortcuts::view(app));
        }
    }

    if app.view == View::Shortcuts && app.edit_rule.is_some() {
        shell = shell.push(shortcuts::editor(app));
    }

    if app.view == View::Keyboard && app.selected.is_some() {
        shell = shell.push(editor::key_editor(app));
    }

    if let Some(toast) = &app.toast {
        shell = shell.push(overlays::toast(app, toast));
    }

    // `.app-shell`: the rounded, bordered card holding the whole UI.
    let shell = container(shell)
        .width(Length::Fill)
        .class(ctheme::Container::custom(|_| container::Style {
            background: Some(surface().into()),
            border: Border {
                color: white(0.09),
                width: 1.0,
                radius: 14.0.into(),
            },
            shadow: Shadow {
                color: black(0.75),
                offset: Vector::new(0.0, 40.0),
                blur_radius: 90.0,
            },
            ..container::Style::default()
        }));

    // `.app-stage`: dark page background with the shell centered.
    let stage = container(
        container(shell)
            .width(Length::Fill)
            .max_width(SHELL_MAX_WIDTH),
    )
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .padding([28, 20]);

    let page = container(widget::scrollable(stage))
        .width(Length::Fill)
        .height(Length::Fill)
        .class(ctheme::Container::custom(|_| container::Style {
            background: Some(theme::bg().into()),
            ..container::Style::default()
        }));

    let mut layers: Vec<Element<'_, Message>> = vec![page.into()];

    if app.view == View::Keyboard && app.remaps_open {
        layers.push(overlays::remaps_dialog(app));
    }
    if app.view == View::Keyboard && app.selected.is_some() && app.capture {
        layers.push(overlays::capture_dialog(app));
    }
    if app.onboarding && app.view != View::Tester {
        layers.push(overlays::onboarding(app));
    }

    stack(layers)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

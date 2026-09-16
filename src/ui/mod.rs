//! The Keyloom interface, following the design export in `design/`.

pub mod editor;
pub mod header;
pub mod keyboard_view;
pub mod model;
pub mod overlays;
pub mod shortcuts;
pub mod tester;
pub mod theme;

use cosmic::iced::core::text::LineHeight;
use cosmic::iced::font::Weight;
use cosmic::iced::widget::stack;
use cosmic::iced::{Alignment, Border, Color, Font, Length, Shadow, Vector};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message, View};
use theme::{oklch, shadow, surface, tint, white};

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
                color: shadow(0.7),
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
        Pill::Key => (Some(tint(0.31, 0.04)), tint(0.44, 0.07), tint(0.95, 0.02)),
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

/// Build the window content: the app fills the window like a native
/// application — selection editors slide in as a bottom sheet, dialogs
/// use the app's dialog slot.
pub fn view(app: &App) -> Element<'_, Message> {
    let mut shell = widget::column::with_capacity(2);

    match app.view {
        View::Keyboard | View::Tester => {
            shell = shell.push(keyboard_view::device_toolbar(app));
            shell = shell.push(keyboard_view::area(app));
        }
        View::Shortcuts => {
            // The rule list can outgrow the window; scroll it natively.
            shell = shell.push(widget::scrollable(shortcuts::view(app)));
        }
    }

    let page = container(shell)
        .width(Length::Fill)
        .height(Length::Fill)
        .class(ctheme::Container::custom(|_| container::Style {
            background: Some(surface().into()),
            ..container::Style::default()
        }));

    // Bottom overlays: the confirmation toast stacked above the editor
    // sheet, both anchored to the window bottom.
    let sheet = bottom_sheet(app);
    let toast = app.toast.as_ref().map(|toast| overlays::toast(app, toast));
    if sheet.is_none() && toast.is_none() {
        return page.into();
    }

    let mut bottom = widget::column::with_capacity(2).width(Length::Fill);
    if let Some(toast) = toast {
        bottom = bottom.push(toast);
    }
    if let Some(sheet) = sheet {
        bottom = bottom.push(sheet);
    }
    let bottom = container(bottom)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::End);

    stack([page.into(), bottom.into()])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The editor sheet sliding over the bottom of the window — the app's
/// context drawer, repositioned to the bottom edge.
fn bottom_sheet(app: &App) -> Option<Element<'_, Message>> {
    let (title, close, content, footer): (String, _, Element<'_, Message>, _) = if app.view
        == View::Keyboard
        && let Some(selected) = app.selected
    {
        (
            match app.active_layer() {
                Some(layer) => format!(
                    "While holding {}, make {} act as…",
                    model::key_name(&layer.trigger),
                    model::key_name(selected)
                ),
                None => format!("Make {} act as…", model::key_name(selected)),
            },
            Message::ClosePanel,
            editor::key_editor(app),
            editor::key_editor_footer(app),
        )
    } else if app.view == View::Shortcuts
        && let Some(edit) = app.edit_rule
    {
        (
            if edit.rule.is_some() {
                "Edit shortcut".to_owned()
            } else {
                "New shortcut".to_owned()
            },
            Message::CloseEdit,
            shortcuts::rule_editor(app),
            shortcuts::rule_editor_footer(app),
        )
    } else {
        return None;
    };

    let header = widget::row::with_capacity(3)
        .align_y(Alignment::Center)
        .push(txt_semibold(title, 16.0, theme::fg()))
        .push(widget::Space::new().width(Length::Fill))
        .push(
            widget::button::custom(txt("Close", 13.0, oklch(0.95, 0.01, 152.0)))
                .class(theme::quiet(false))
                .padding([8, 13])
                .on_press(close),
        );

    let panel = widget::column::with_capacity(3)
        .spacing(14)
        .push(header)
        .push(
            // Bound the content so the footer always stays visible.
            container(widget::scrollable(container(content).padding([0, 4])))
                .max_height(SHEET_CONTENT_MAX_HEIGHT),
        )
        .push(footer);

    let sheet: Element<'_, Message> = container(
        container(panel)
            .width(Length::Fill)
            .padding([18, 24])
            .class(ctheme::Container::custom(|_| container::Style {
                background: Some(oklch(0.185, 0.007, 152.0).into()),
                border: Border {
                    color: white(0.12),
                    width: 1.0,
                    radius: [14.0, 14.0, 0.0, 0.0].into(),
                },
                shadow: Shadow {
                    color: shadow(0.6),
                    offset: Vector::new(0.0, -18.0),
                    blur_radius: 48.0,
                },
                ..container::Style::default()
            })),
    )
    .width(Length::Fill)
    .padding([0, 10])
    .into();

    // Rise/fall animation (the design's `kbRise`): reveal the sheet from
    // the bottom edge by growing a clip window, then reverse the same
    // eased motion when closing. The sheet stays top-aligned inside it,
    // so its header moves like a translate.
    let progress = app.sheet_progress();
    if progress >= 1.0 {
        return Some(sheet);
    }
    let eased = 1.0 - (1.0 - progress).powi(3);
    Some(
        container(sheet)
            .width(Length::Fill)
            .max_height(eased * SHEET_RISE_EXTENT)
            .clip(true)
            .into(),
    )
}

/// Tallest the sheet's scrolling content region may grow.
const SHEET_CONTENT_MAX_HEIGHT: f32 = 300.0;

/// Height swept by the rise animation — at least the tallest sheet.
const SHEET_RISE_EXTENT: f32 = 460.0;

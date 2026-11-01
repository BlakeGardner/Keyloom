//! The window list: every window open on the desktop, exactly as
//! `xremap --list-windows` reports it ([`crate::apps::watch_windows`]),
//! asked again every few seconds in a window of its own. When an
//! application is missing from the application picker, or a remap for
//! it does not apply, this shows the id remapping actually sees.
//! Every value can be selected and copied (right-click offers Copy).

use cosmic::Application;
use cosmic::iced::core::text::Wrapping;
use cosmic::iced::{Alignment, Border, Color, Font, Length, Size, window};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message};
use crate::apps::{self, Window};
use crate::ui::keyboard_view::rule;
use crate::ui::theme::{ghost_button, muted, oklch, success, white};
use crate::ui::{eyebrow, hspace, mono, txt, txt_semibold};

/// The window's title.
pub const TITLE: &str = "Window List";

/// The window's size when it opens, in logical pixels.
pub const SIZE: Size = Size::new(760.0, 480.0);

/// The smallest the window can be made.
pub const MIN_SIZE: Size = Size::new(480.0, 300.0);

/// How the window is made ([`crate::ui::side_window_settings`]).
pub fn settings() -> window::Settings {
    crate::ui::side_window_settings(SIZE, MIN_SIZE)
}

/// The window list while it is open.
#[derive(Clone, Debug)]
pub struct WindowListView {
    pub id: window::Id,
    /// xremap's latest answer, once one came.
    pub list: Option<apps::WindowList>,
    /// The rows were just copied, which the Copy button acknowledges
    /// for a moment.
    pub copied: bool,
    /// Copies made so far, so only the latest one's acknowledgment
    /// ends it.
    copies: u64,
}

impl WindowListView {
    pub fn new(id: window::Id) -> Self {
        Self {
            id,
            list: None,
            copied: false,
            copies: 0,
        }
    }

    /// The windows of the latest answer, when it listed any.
    fn windows(&self) -> &[Window] {
        match &self.list {
            Some(apps::WindowList {
                windows: Ok(windows),
                ..
            }) => windows,
            _ => &[],
        }
    }

    /// Count a copy, returning its number for the acknowledgment to end.
    pub fn copy(&mut self) -> u64 {
        self.copies += 1;
        self.copied = true;
        self.copies
    }

    /// End the acknowledgment of copy number `copy`, unless another
    /// copy came after it.
    pub fn copy_shown(&mut self, copy: u64) {
        if self.copies == copy {
            self.copied = false;
        }
    }

    /// The rows, as the plain text Copy puts on the clipboard: xremap's
    /// application id and title columns, separated by tabs.
    pub fn text(&self) -> String {
        std::iter::once("APP_CLASS\tTITLE\n".to_owned())
            .chain(
                self.windows()
                    .iter()
                    .map(|window| format!("{}\t{}\n", window.app_id, window.title)),
            )
            .collect()
    }
}

/// The whole window: its header, then the list.
pub fn window<'a>(app: &'a App, view: &'a WindowListView) -> Element<'a, Message> {
    let focused = app.core().focused_window() == Some(view.id);
    let mut header = widget::header_bar()
        .title(TITLE)
        .focused(focused)
        .on_drag(Message::WindowListDrag)
        .on_double_click(Message::WindowListMaximize)
        .on_close(Message::WindowListClose);
    if cosmic::config::show_maximize() {
        header = header.on_maximize(Message::WindowListMaximize);
    }
    if cosmic::config::show_minimize() {
        header = header.on_minimize(Message::WindowListMinimize);
    }

    crate::ui::side_window(
        header,
        widget::column::with_capacity(4)
            .push(toolbar(view))
            .push(source(view))
            .push_maybe(banner(view))
            .push(rows(view)),
    )
}

/// The toolbar's height, the same with its button or without, so the
/// list does not shift when the first rows bring it.
const TOOLBAR_HEIGHT: f32 = 50.0;

/// Whether the list is up to date, and copying it.
fn toolbar(view: &WindowListView) -> Element<'_, Message> {
    let (dot_color, label) = match &view.list {
        None => (muted(), "Asking for the open windows…".to_owned()),
        Some(apps::WindowList { windows: Ok(_), .. }) => (
            success(),
            format!("Updating every {} seconds", apps::REFRESH.as_secs()),
        ),
        Some(apps::WindowList {
            windows: Err(_), ..
        }) => (oklch(0.62, 0.19, 25.0), "Not listed".to_owned()),
    };
    let dot = container(widget::Space::new().width(7.0).height(7.0)).class(
        ctheme::Container::custom(move |_| container::Style {
            background: Some(dot_color.into()),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }),
    );

    let row = widget::row::with_capacity(4)
        .spacing(8)
        .padding([0, 14])
        .height(Length::Fixed(TOOLBAR_HEIGHT))
        .align_y(Alignment::Center)
        .push(dot)
        .push(txt(label, 12.0, oklch(0.85, 0.01, 152.0)))
        .push(hspace());
    if view.windows().is_empty() {
        return row.into();
    }
    row.push(
        widget::button::custom(txt_semibold(
            if view.copied { "Copied" } else { "Copy" },
            12.0,
            oklch(0.9, 0.01, 152.0),
        ))
        .class(ghost_button())
        .padding([6, 12])
        .on_press(Message::WindowListCopy),
    )
    .into()
}

/// Where the rows come from, and which column remapping matches on.
fn source(view: &WindowListView) -> Element<'_, Message> {
    let command = view
        .list
        .as_ref()
        .and_then(|list| list.binary.as_ref())
        .map_or_else(
            || "xremap --list-windows".to_owned(),
            |binary| format!("{} --list-windows", binary.display()),
        );
    container(
        widget::column::with_capacity(2)
            .spacing(4)
            .push(txt(
                "Every window the desktop reports, exactly as remapping sees it. \
                 Application-specific remaps match the application id.",
                12.0,
                muted(),
            ))
            .push(mono(command, 11.5, oklch(0.7, 0.01, 152.0))),
    )
    .padding([0, 16, 10, 16])
    .width(Length::Fill)
    .into()
}

/// Why no windows are listed, when xremap could not list them.
fn banner(view: &WindowListView) -> Option<Element<'_, Message>> {
    let Some(apps::WindowList {
        windows: Err(error),
        ..
    }) = &view.list
    else {
        return None;
    };
    let card = widget::column::with_capacity(3)
        .spacing(4)
        .width(Length::Fill)
        .push(txt_semibold(
            "The open windows can't be listed",
            13.0,
            oklch(0.93, 0.01, 152.0),
        ))
        .push(
            widget::text(error.as_str())
                .font(cosmic::iced::Font::MONOSPACE)
                .size(12.0)
                .class(ctheme::Text::Color(oklch(0.85, 0.01, 152.0))),
        )
        .push(txt(
            format!(
                "Keyloom asks again every {} seconds.",
                apps::REFRESH.as_secs()
            ),
            11.5,
            muted(),
        ));
    Some(
        container(container(card).padding([12, 14]).width(Length::Fill).class(
            ctheme::Container::custom(|_| container::Style {
                background: Some(oklch(0.27, 0.035, 25.0).into()),
                border: Border {
                    color: oklch(0.45, 0.09, 25.0),
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..container::Style::default()
            }),
        ))
        .padding([0, 14, 10, 14])
        .into(),
    )
}

/// The columns' widths: the id and the title share the window equally,
/// so a reverse-DNS id fits on one line at the size the window opens
/// at.
const ID_WIDTH: Length = Length::FillPortion(1);
const TITLE_WIDTH: Length = Length::FillPortion(1);

/// The rows under their column names, or what stands in for them.
fn rows(view: &WindowListView) -> Element<'_, Message> {
    let windows = view.windows();
    if windows.is_empty() {
        let text = match &view.list {
            None => "Asking for the open windows…",
            Some(apps::WindowList { windows: Ok(_), .. }) => "The desktop reports no windows.",
            // The banner says why.
            Some(apps::WindowList {
                windows: Err(_), ..
            }) => "",
        };
        return container(txt(text, 13.0, muted()))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
    }

    // Padded like the rows, which leave room for the scrollbar.
    let heading = widget::row::with_capacity(2)
        .spacing(12)
        .padding([0, 14, 0, 0])
        .push(eyebrow("Application id").width(ID_WIDTH))
        .push(eyebrow("Title").width(TITLE_WIDTH));
    let list = windows.iter().fold(
        widget::column::with_capacity(windows.len()).spacing(2),
        |list, window| list.push(row(window)),
    );
    widget::column::with_capacity(3)
        .spacing(6)
        .padding([0, 16, 0, 16])
        .push(heading)
        .push(rule(white(0.09)))
        .push(
            // Room on the right for the scrollbar, clear of the titles.
            widget::scrollable(list.padding([0, 14, 16, 0]))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .into()
}

/// Text that can be selected and copied, with right-click offering
/// Copy and Select All: in the interface's face, or `font`.
fn selectable(content: &str, size: f32, color: Color, font: Option<Font>) -> Element<'_, Message> {
    widget::selectable_text(content)
        .font_maybe(font)
        .size(size)
        .class(ctheme::Text::Color(color))
        // Ids have no spaces to break at; they break anywhere rather
        // than run into the next column.
        .wrapping(Wrapping::WordOrGlyph)
        .width(Length::Fill)
        .into()
}

/// One window: its application id and title, each selectable.
fn row(window: &Window) -> Element<'_, Message> {
    let id = if !window.app_id.is_empty() {
        selectable(
            &window.app_id,
            12.0,
            oklch(0.92, 0.01, 152.0),
            Some(Font::MONOSPACE),
        )
    } else {
        txt("none", 12.0, muted()).into()
    };
    widget::row::with_capacity(2)
        .spacing(12)
        .padding([2, 0])
        .align_y(Alignment::Center)
        .push(container(id).width(ID_WIDTH))
        .push(
            container(selectable(
                &window.title,
                12.0,
                oklch(0.85, 0.01, 152.0),
                None,
            ))
            .width(TITLE_WIDTH),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn window(app_id: &str, title: &str) -> Window {
        Window {
            app_id: app_id.to_owned(),
            title: title.to_owned(),
        }
    }

    fn open() -> WindowListView {
        WindowListView::new(window::Id::unique())
    }

    #[test]
    fn the_copied_text_is_xremaps_columns_one_row_per_window() {
        let mut view = open();
        assert_eq!(view.text(), "APP_CLASS\tTITLE\n", "nothing yet");

        view.list = Some(apps::WindowList {
            binary: Some(PathBuf::from("/usr/bin/xremap")),
            windows: Ok(vec![
                window("firefox", "Keyloom · GitHub — Firefox"),
                window("", "untitled"),
            ]),
        });

        assert_eq!(
            view.text(),
            "APP_CLASS\tTITLE\n\
             firefox\tKeyloom · GitHub — Firefox\n\
             \tuntitled\n"
        );
    }

    #[test]
    fn a_failed_answer_lists_no_windows() {
        let mut view = open();
        view.list = Some(apps::WindowList {
            binary: None,
            windows: Err("xremap is not installed".to_owned()),
        });

        assert!(view.windows().is_empty());
    }

    #[test]
    fn only_the_latest_copy_ends_the_acknowledgment() {
        let mut view = open();

        let first = view.copy();
        let second = view.copy();
        view.copy_shown(first);
        assert!(view.copied);
        view.copy_shown(second);
        assert!(!view.copied);
    }
}

//! The remapping log: the service's journal ([`crate::journal`]),
//! followed live in a window of its own, so a failure can be looked
//! into beside the main window and without a terminal.

use std::collections::VecDeque;

use cosmic::Application;
use cosmic::iced::widget::scrollable::Viewport;
use cosmic::iced::{Alignment, Border, Color, Font, Length, Size, window};
use cosmic::widget::{self, container};
use cosmic::{Element, theme as ctheme};

use crate::app::{App, Message};
use crate::config;
use crate::journal::{self, End, Entry, Event, Severity, Source};
use crate::ui::keyboard_view::rule;
use crate::ui::theme::{ghost_button, muted, oklch, success, surface, white};
use crate::ui::{hspace, txt, txt_semibold};

/// The window's title.
pub const TITLE: &str = "Remapping Log";

/// The window's size when it opens, in logical pixels.
pub const SIZE: Size = Size::new(820.0, 560.0);

/// The smallest the window can be made.
pub const MIN_SIZE: Size = Size::new(480.0, 320.0);

/// Most entries kept, and drawn: as many as the backlog, the oldest
/// giving way as new ones arrive. Every update draws the window anew,
/// so this bounds what an open log adds to it.
const KEEP: usize = journal::BACKLOG;

/// Most of journalctl's notices kept: the latest say what matters.
const NOTICES: usize = 3;

/// How the window is made: like the main window, it draws its own
/// header and corners (libcosmic's client-side decorations), so it
/// looks the same on every desktop, and it carries the application's
/// id so the desktop counts it as Keyloom's.
pub fn settings() -> window::Settings {
    let mut settings = window::Settings {
        size: SIZE,
        min_size: Some(MIN_SIZE),
        decorations: false,
        transparent: true,
        resizable: true,
        resize_border: 8,
        ..window::Settings::default()
    };
    settings.platform_specific.application_id = config::APP_ID.to_owned();
    settings
}

/// The scrolling list of entries, for keeping it at the latest one.
pub fn scroll_id() -> widget::Id {
    widget::Id::new("log-entries")
}

/// Where reading the log stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading {
    /// journalctl is starting, and nothing has come of it yet.
    Starting,
    /// Entries arrive as they are written.
    Following,
    /// Following stopped.
    Ended(End),
}

/// The log window while it is open.
#[derive(Clone, Debug)]
pub struct LogWindow {
    pub id: window::Id,
    /// What has been read, oldest first.
    entries: VecDeque<Entry>,
    pub reading: Reading,
    /// What journalctl said about itself, latest last.
    pub notices: Vec<String>,
    /// Attempts at reading the log so far: "Try again" starts afresh.
    pub attempt: u64,
    /// Whether the list is scrolled to its end, where it stays as new
    /// entries arrive. Scrolled back, it stays where it was.
    pub at_end: bool,
    /// The entries were just copied, which the Copy button
    /// acknowledges for a moment.
    pub copied: bool,
    /// Copies made so far, so only the latest one's acknowledgment
    /// ends it.
    copies: u64,
}

impl LogWindow {
    pub fn new(id: window::Id) -> Self {
        Self {
            id,
            entries: VecDeque::new(),
            reading: Reading::Starting,
            notices: Vec::new(),
            attempt: 0,
            at_end: true,
            copied: false,
            copies: 0,
        }
    }

    /// Take in what following the log reported. Returns whether new
    /// entries arrived.
    pub fn receive(&mut self, event: Event) -> bool {
        match event {
            Event::Following => {
                self.reading = Reading::Following;
                false
            }
            Event::Entries(entries) => {
                self.reading = Reading::Following;
                self.entries.extend(entries);
                let excess = self.entries.len().saturating_sub(KEEP);
                self.entries.drain(..excess);
                true
            }
            Event::Notice(notice) => {
                self.notices.push(notice);
                let excess = self.notices.len().saturating_sub(NOTICES);
                self.notices.drain(..excess);
                false
            }
            Event::Ended(end) => {
                self.reading = Reading::Ended(end);
                false
            }
        }
    }

    /// Start reading the log afresh.
    pub fn retry(&mut self) {
        self.entries.clear();
        self.notices.clear();
        self.reading = Reading::Starting;
        self.attempt += 1;
        self.at_end = true;
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

    /// Whether older entries are left out. Holding as many as it keeps,
    /// the window has either let the oldest go as new ones came, or
    /// read a backlog that stopped short of the journal's start (unless
    /// the journal held exactly that many).
    pub fn has_earlier(&self) -> bool {
        self.entries.len() >= KEEP
    }

    /// The entries, as the plain text Copy puts on the clipboard.
    pub fn text(&self) -> String {
        self.entries
            .iter()
            .map(|entry| entry.to_line() + "\n")
            .collect()
    }
}

/// Whether a scroll left the list at its end, within a pixel or two.
pub fn at_end(viewport: Viewport) -> bool {
    viewport.absolute_offset_reversed().y <= 2.0
}

/// Red for errors, in text: lighter than the status dot's red, to read
/// on the dark surface.
fn error_text() -> Color {
    oklch(0.74, 0.15, 25.0)
}

/// Amber for warnings, the header's "holding" color.
fn warning_text() -> Color {
    oklch(0.8, 0.13, 85.0)
}

/// The whole window: its header, then the log.
pub fn window<'a>(app: &'a App, log: &'a LogWindow) -> Element<'a, Message> {
    let focused = app.core().focused_window() == Some(log.id);
    let mut header = widget::header_bar()
        .title(TITLE)
        .focused(focused)
        .on_drag(Message::LogDrag)
        .on_double_click(Message::LogMaximize)
        .on_close(Message::LogClose);
    if cosmic::config::show_maximize() {
        header = header.on_maximize(Message::LogMaximize);
    }
    if cosmic::config::show_minimize() {
        header = header.on_minimize(Message::LogMinimize);
    }

    let content = container(
        widget::column::with_capacity(3)
            .push(toolbar(log))
            .push_maybe(banner(log))
            .push(entries(log)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .class(ctheme::Container::custom(|theme| {
        // Rounded where it meets the frame's corners, beneath the header.
        let [.., bottom_right, bottom_left] = corners(theme).map(|radius| (radius - 1.0).max(0.0));
        container::Style {
            background: Some(surface().into()),
            border: Border {
                radius: [0.0, 0.0, bottom_right, bottom_left].into(),
                ..Border::default()
            },
            ..container::Style::default()
        }
    }));

    // The frame libcosmic gives its main window: a hairline border with
    // the theme's corners, around the header and the content.
    container(widget::column::with_capacity(2).push(header).push(content))
        .padding(1)
        .class(ctheme::Container::custom(|theme| {
            let cosmic = theme.cosmic();
            container::Style {
                background: Some(Color::from(cosmic.background(theme.transparent).base).into()),
                border: Border {
                    color: cosmic.bg_divider().into(),
                    width: 1.0,
                    radius: corners(theme).into(),
                },
                ..container::Style::default()
            }
        }))
        .into()
}

/// The window's corner radii, as libcosmic rounds its main window: the
/// theme's small radius, widened to sit around the content's.
fn corners(theme: &cosmic::Theme) -> [f32; 4] {
    theme
        .cosmic()
        .radius_s()
        .map(|radius| if radius < 4.0 { radius } else { radius + 4.0 })
}

/// The toolbar's height, the same with its buttons or without, so the
/// log does not shift when the first entries bring them.
const TOOLBAR_HEIGHT: f32 = 50.0;

/// Whether the log is being followed, and what can be done with it.
fn toolbar(log: &LogWindow) -> Element<'_, Message> {
    let (dot_color, label) = match log.reading {
        Reading::Starting => (muted(), "Reading the log…"),
        Reading::Following => (success(), "Following live"),
        Reading::Ended(_) => (oklch(0.62, 0.19, 25.0), "Stopped"),
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

    let mut row = widget::row::with_capacity(6)
        .spacing(8)
        .padding([0, 14])
        .height(Length::Fixed(TOOLBAR_HEIGHT))
        .align_y(Alignment::Center)
        .push(dot)
        .push(txt(label, 12.0, oklch(0.85, 0.01, 152.0)))
        .push(hspace());
    // Nothing read, nothing to show or copy.
    if log.entries.is_empty() {
        return row.into();
    }

    if !log.at_end {
        row = row.push(
            widget::button::custom(txt_semibold(
                "Jump to latest",
                12.0,
                oklch(0.9, 0.01, 152.0),
            ))
            .class(ghost_button())
            .padding([6, 10])
            .on_press(Message::LogJumpToEnd),
        );
    }

    let copy = widget::button::custom(txt_semibold(
        if log.copied { "Copied" } else { "Copy" },
        12.0,
        oklch(0.9, 0.01, 152.0),
    ))
    .class(ghost_button())
    .padding([6, 12])
    .on_press(Message::LogCopy);
    row.push(copy).into()
}

/// What stands in the way of following the log, or what journalctl
/// said about itself, above the entries.
fn banner(log: &LogWindow) -> Option<Element<'_, Message>> {
    let said = || {
        log.notices.iter().fold(
            widget::column::with_capacity(log.notices.len()),
            |column, notice| {
                column.push(
                    widget::text(notice.as_str())
                        .font(Font::MONOSPACE)
                        .size(12.0)
                        .class(ctheme::Text::Color(oklch(0.85, 0.01, 152.0))),
                )
            },
        )
    };
    let (title, body): (&str, Element<'_, Message>) = match &log.reading {
        Reading::Ended(End::Missing) => (
            "The log can't be read",
            txt(
                "Keyloom reads the log with journalctl, which isn't installed on this system.",
                12.0,
                muted(),
            )
            .into(),
        ),
        Reading::Ended(End::NotStarted(reason)) => (
            "The log can't be read",
            txt(
                format!("journalctl could not be started: {reason}"),
                12.0,
                muted(),
            )
            .into(),
        ),
        Reading::Ended(End::Exited(code)) => {
            let mut body = widget::column::with_capacity(3).spacing(6);
            if log.notices.is_empty() {
                body = body.push(txt(
                    match code {
                        Some(code) => format!("journalctl stopped with status {code}."),
                        None => "journalctl was stopped.".to_owned(),
                    },
                    12.0,
                    muted(),
                ));
            } else {
                body = body.push(said());
            }
            body = body.push(txt(
                format!(
                    "Where the journal is kept only in memory, reading it usually takes \
                     membership in the systemd-journal group. In a terminal: {}",
                    journal::terminal_command()
                ),
                11.5,
                muted(),
            ));
            ("The log stopped updating", body.into())
        }
        Reading::Starting | Reading::Following if !log.notices.is_empty() => {
            return Some(
                container(said())
                    .padding([0, 14, 8, 14])
                    .width(Length::Fill)
                    .into(),
            );
        }
        Reading::Starting | Reading::Following => return None,
    };

    let card = widget::row::with_capacity(2)
        .spacing(16)
        .align_y(Alignment::Center)
        .push(
            widget::column::with_capacity(2)
                .spacing(4)
                .width(Length::Fill)
                .push(txt_semibold(title, 13.0, oklch(0.93, 0.01, 152.0)))
                .push(body),
        )
        .push(
            widget::button::custom(txt_semibold("Try again", 12.0, oklch(0.92, 0.01, 152.0)))
                .class(ghost_button())
                .padding([6, 12])
                .on_press(Message::LogRetry),
        );
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

/// The entries, grouped by run, or what stands in for them.
fn entries(log: &LogWindow) -> Element<'_, Message> {
    if log.entries.is_empty() {
        return empty(log);
    }

    let mut column = widget::column::with_capacity(log.entries.len() + 16)
        .spacing(2)
        .padding([2, 18, 16, 16]);
    if log.has_earlier() {
        column = column.push(
            container(txt(
                format!(
                    "Older entries are left out here; {} shows them all.",
                    journal::terminal_command()
                ),
                11.5,
                muted(),
            ))
            .padding([6, 0, 0, 0]),
        );
    }
    let mut previous: Option<&Entry> = None;
    for entry in &log.entries {
        // A new run, or a run carrying on past midnight, gets a heading
        // with the date its entries are from.
        let heading = previous.is_none_or(|previous| {
            (entry.run.is_some() && previous.run != entry.run)
                || previous.at.date() != entry.at.date()
        });
        if heading {
            column = column.push(run_heading(entry, previous.is_none()));
        }
        column = column.push(line(entry));
        previous = Some(entry);
    }

    widget::scrollable(column)
        .id(scroll_id())
        .on_scroll(|viewport| Message::LogScrolled(at_end(viewport)))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The heading over a run's entries: when it starts, and a rule.
fn run_heading(entry: &Entry, first: bool) -> Element<'static, Message> {
    widget::row::with_capacity(2)
        .spacing(10)
        .padding([if first { 6 } else { 14 }, 0, 4, 0])
        .align_y(Alignment::Center)
        .push(txt_semibold(
            entry.at.strftime("%a, %b %-d · %H:%M").to_string(),
            11.0,
            muted(),
        ))
        .push(rule(white(0.09)))
        .into()
}

/// One entry: its time, then its text, colored by what it calls for.
fn line(entry: &Entry) -> Element<'_, Message> {
    let color = match entry.severity() {
        Severity::Error => error_text(),
        Severity::Warning => warning_text(),
        // systemd's notes about starting and stopping frame xremap's own
        // output rather than compete with it.
        Severity::Normal if entry.source == Source::Systemd => muted(),
        Severity::Normal => oklch(0.88, 0.01, 152.0),
    };
    widget::row::with_capacity(2)
        .spacing(12)
        .push(
            widget::text(entry.clock.as_str())
                .font(Font::MONOSPACE)
                .size(12.0)
                .class(ctheme::Text::Color(oklch(0.6, 0.01, 152.0)))
                .width(Length::Fixed(62.0)),
        )
        .push(
            widget::text(entry.text.as_str())
                .font(Font::MONOSPACE)
                .size(12.0)
                .class(ctheme::Text::Color(color))
                .width(Length::Fill),
        )
        .into()
}

/// What the list shows with nothing to draw.
fn empty(log: &LogWindow) -> Element<'_, Message> {
    let (title, body) = match log.reading {
        Reading::Starting => ("Reading the log…", None),
        // The banner says why.
        Reading::Ended(_) => ("", None),
        Reading::Following => (
            "Nothing logged yet",
            Some("Lines appear here as the remapping service writes them."),
        ),
    };
    let mut column = widget::column::with_capacity(2)
        .spacing(6)
        .align_x(Alignment::Center);
    if !title.is_empty() {
        column = column.push(txt_semibold(title, 14.0, oklch(0.9, 0.01, 152.0)));
    }
    if let Some(body) = body {
        column = column.push(txt(body, 12.0, muted()));
    }
    container(column)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    /// An entry from xremap in `run`, at `hour`:`minute` on October 2.
    fn entry(text: &str, run: &str, hour: i8, minute: i8) -> Entry {
        let at = date(2026, 10, 2).at(hour, minute, 0, 0);
        Entry {
            at,
            clock: format!("{hour:02}:{minute:02}:00"),
            text: text.to_owned(),
            writer: "xremap".to_owned(),
            source: Source::Remapper,
            priority: 6,
            run: Some(run.to_owned()),
        }
    }

    fn open() -> LogWindow {
        LogWindow::new(window::Id::unique())
    }

    #[test]
    fn the_log_starts_reading_scrolled_to_its_end() {
        let log = open();

        assert_eq!(log.reading, Reading::Starting);
        assert!(log.at_end);
        assert!(log.entries.is_empty());
        assert!(!log.has_earlier());
    }

    #[test]
    fn entries_arrive_in_order_and_mean_the_log_is_followed() {
        let mut log = open();

        assert!(log.receive(Event::Entries(vec![entry("one", "a", 9, 0)])));
        assert!(log.receive(Event::Entries(vec![entry("two", "a", 9, 1)])));

        assert_eq!(log.reading, Reading::Following);
        let texts: Vec<&str> = log.entries.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, ["one", "two"]);
    }

    #[test]
    fn a_quiet_log_is_followed_with_nothing_to_show() {
        let mut log = open();

        assert!(!log.receive(Event::Following));

        assert_eq!(log.reading, Reading::Following);
        assert!(log.entries.is_empty());
    }

    #[test]
    fn the_oldest_entries_give_way_beyond_what_is_kept() {
        let mut log = open();
        let many: Vec<Entry> = (0..KEEP + 5)
            .map(|n| entry(&n.to_string(), "a", 9, 0))
            .collect();

        log.receive(Event::Entries(many));

        assert_eq!(log.entries.len(), KEEP);
        assert_eq!(log.entries.front().map(|e| e.text.as_str()), Some("5"));
        assert_eq!(
            log.entries.back().map(|e| e.text.clone()),
            Some((KEEP + 4).to_string())
        );
        assert!(log.has_earlier(), "the window says older ones are left out");
    }

    #[test]
    fn the_copied_text_is_one_line_per_entry() {
        let mut log = open();
        log.receive(Event::Entries(vec![
            entry("one", "a", 9, 0),
            entry("two", "b", 9, 5),
        ]));

        assert_eq!(
            log.text(),
            "2026-10-02 09:00:00 xremap: one\n2026-10-02 09:05:00 xremap: two\n"
        );
    }

    #[test]
    fn notices_keep_the_latest_few_and_the_end_is_remembered() {
        let mut log = open();
        for n in 0..5 {
            log.receive(Event::Notice(format!("notice {n}")));
        }
        log.receive(Event::Ended(End::Exited(Some(1))));

        assert_eq!(log.notices, ["notice 2", "notice 3", "notice 4"]);
        assert_eq!(log.reading, Reading::Ended(End::Exited(Some(1))));
    }

    #[test]
    fn trying_again_starts_afresh() {
        let mut log = open();
        log.receive(Event::Entries(vec![entry("one", "a", 9, 0)]));
        log.receive(Event::Notice("a warning".to_owned()));
        log.receive(Event::Ended(End::Exited(Some(1))));
        log.at_end = false;

        log.retry();

        assert_eq!(log.attempt, 1, "a new attempt follows the log anew");
        assert_eq!(log.reading, Reading::Starting);
        assert!(log.entries.is_empty() && log.notices.is_empty());
        assert!(log.at_end);
    }

    #[test]
    fn only_the_latest_copy_ends_the_acknowledgment() {
        let mut log = open();

        let first = log.copy();
        let second = log.copy();
        log.copy_shown(first);
        assert!(log.copied);
        log.copy_shown(second);
        assert!(!log.copied);
    }
}

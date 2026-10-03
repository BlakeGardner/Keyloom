//! The remapping service's log: the systemd journal's entries for
//! Keyloom's unit, followed live for the log window (`ui::log`).
//!
//! journalctl does the reading, as it does for `systemctl status` and
//! for a terminal. journald has no D-Bus request for reading the
//! journal, its Varlink interface for that (`io.systemd.JournalAccess`)
//! only arrived in systemd 260, and libsystemd's own reader would make
//! every package build against the C library. journalctl comes with
//! systemd, so it is there wherever the unit can be. Its JSON output
//! carries each entry's fields as journald stored them, which is how
//! xremap's own output is told apart from systemd's messages about the
//! unit, and one run of the unit from the next.

use std::io;
use std::pin::pin;
use std::process::Stdio;
use std::time::Duration;

use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{SinkExt, Stream, StreamExt, stream};
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;

use crate::service::UNIT;

/// The program that reads the journal.
const JOURNALCTL: &str = "journalctl";

/// How many of the latest entries are read when the log opens. Every
/// restart of the unit writes a few dozen lines, so this reaches back
/// over dozens of runs.
pub const BACKLOG: usize = 1000;

/// The fields read from each entry. journalctl adds the timestamps
/// whatever is asked for.
const FIELDS: &str = "MESSAGE,PRIORITY,SYSLOG_IDENTIFIER,_SYSTEMD_INVOCATION_ID,USER_INVOCATION_ID";

/// How long journalctl may stay silent before the log counts as
/// having nothing to show: a backlog arrives at once.
const QUIET: Duration = Duration::from_millis(400);

/// Most lines handed over at once: the backlog arrives in a few
/// batches rather than one message per entry.
const BATCH: usize = 512;

/// The command a person would run in a terminal to read the same log.
pub fn terminal_command() -> String {
    format!("journalctl --user -u {UNIT}")
}

/// journalctl's arguments for following the unit's log: the latest
/// [`BACKLOG`] entries, then each new one as it is written, as JSON.
/// `--user` reads only the user's own journal, where the unit and the
/// user manager write, so no warning about the system journal comes
/// with it; `--all` keeps long messages that JSON output would
/// otherwise leave out.
pub fn arguments() -> Vec<String> {
    vec![
        "--user".to_owned(),
        format!("--unit={UNIT}"),
        "--follow".to_owned(),
        format!("--lines={BACKLOG}"),
        "--output=json".to_owned(),
        format!("--output-fields={FIELDS}"),
        "--all".to_owned(),
        "--no-pager".to_owned(),
    ]
}

/// Who wrote an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// xremap itself, on its output.
    Remapper,
    /// systemd's user manager, about the unit: starting and stopping
    /// it, and how a run ended.
    Systemd,
}

/// How much an entry calls for attention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Normal,
    Warning,
    Error,
}

/// One entry of the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// When it was written, in the local time zone.
    pub at: DateTime,
    /// `at`'s time of day, `HH:MM:SS`, as the window shows it.
    pub clock: String,
    pub text: String,
    /// The name it was written under, as journald records it
    /// (`xremap`, `systemd`).
    pub writer: String,
    pub source: Source,
    /// Its syslog priority, 0 (emergency) to 7 (debug).
    pub priority: u8,
    /// The run of the unit it belongs to (systemd's invocation id):
    /// each start, including the one after every applied change, is a
    /// new run.
    pub run: Option<String>,
}

/// A field as journalctl writes it in JSON: text, bytes for a value
/// that is not UTF-8, or every value of a field an entry holds more
/// than once.
#[derive(Deserialize)]
#[serde(untagged)]
enum Field {
    Text(String),
    Bytes(Vec<u8>),
    Several(Vec<Field>),
}

impl Field {
    fn into_text(self) -> String {
        match self {
            Self::Text(text) => text,
            Self::Bytes(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Self::Several(fields) => fields
                .into_iter()
                .next()
                .map(Self::into_text)
                .unwrap_or_default(),
        }
    }
}

/// An entry as journalctl writes it, with the fields Keyloom reads.
#[derive(Deserialize)]
struct Record {
    #[serde(rename = "__REALTIME_TIMESTAMP")]
    realtime: Option<Field>,
    #[serde(rename = "MESSAGE")]
    message: Option<Field>,
    #[serde(rename = "PRIORITY")]
    priority: Option<Field>,
    #[serde(rename = "SYSLOG_IDENTIFIER")]
    identifier: Option<Field>,
    #[serde(rename = "_SYSTEMD_INVOCATION_ID")]
    invocation: Option<Field>,
    /// The run a user manager's message is about.
    #[serde(rename = "USER_INVOCATION_ID")]
    user_invocation: Option<Field>,
}

impl Entry {
    /// The entry on one line of journalctl's JSON output, its time put
    /// in `zone`. A line that is not an entry, or has no time, is
    /// `None`.
    pub fn parse(line: &str, zone: &TimeZone) -> Option<Self> {
        let record: Record = serde_json::from_str(line).ok()?;
        let micros = record.realtime?.into_text().parse::<i64>().ok()?;
        let at = Timestamp::from_microsecond(micros).ok()?;
        let at = Zoned::new(at, zone.clone()).datetime();
        let mut text = record.message.map(Field::into_text).unwrap_or_default();
        text.truncate(text.trim_end().len());
        let writer = record.identifier.map(Field::into_text).unwrap_or_default();
        let manager = record.user_invocation.is_some() || writer == "systemd";
        Some(Self {
            at,
            clock: format!("{:02}:{:02}:{:02}", at.hour(), at.minute(), at.second()),
            text,
            writer,
            source: if manager {
                Source::Systemd
            } else {
                Source::Remapper
            },
            priority: record
                .priority
                .map(Field::into_text)
                .and_then(|priority| priority.parse().ok())
                .unwrap_or(6),
            run: record
                .user_invocation
                .or(record.invocation)
                .map(Field::into_text),
        })
    }

    /// How much the entry calls for attention. xremap writes every line
    /// at the same priority, so its own failures are known by their
    /// wording: the error a Rust program exits with, a panic, and the
    /// operations on a device it could not carry out.
    pub fn severity(&self) -> Severity {
        match self.priority {
            0..=3 => Severity::Error,
            4 => Severity::Warning,
            _ if self.source == Source::Remapper => {
                if self.text.starts_with("Error") || self.text.contains("panicked at") {
                    Severity::Error
                } else if self.text.starts_with("Failed to ") {
                    Severity::Warning
                } else {
                    Severity::Normal
                }
            }
            _ => Severity::Normal,
        }
    }

    /// The entry as a line of plain text, the way a terminal shows it.
    pub fn to_line(&self) -> String {
        let writer = match (self.writer.as_str(), self.source) {
            ("", Source::Remapper) => "xremap",
            ("", Source::Systemd) => "systemd",
            (writer, _) => writer,
        };
        format!(
            "{} {writer}: {}",
            self.at.strftime("%Y-%m-%d %H:%M:%S"),
            self.text
        )
    }
}

/// What following the log reports.
#[derive(Clone, Debug)]
pub enum Event {
    /// journalctl is running and has nothing to show yet: an empty log
    /// writes nothing at all.
    Following,
    /// Entries in the order they were written: the backlog first, in a
    /// few batches, then new ones as they come.
    Entries(Vec<Entry>),
    /// Something journalctl said about itself rather than the log, such
    /// as a journal file it could not read.
    Notice(String),
    /// Following stopped.
    Ended(End),
}

/// Why following the log stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    /// There is no journalctl to run.
    Missing,
    /// journalctl could not be started, for the reason given.
    NotStarted(String),
    /// journalctl exited, with its exit code (`None` when a signal
    /// ended it).
    Exited(Option<i32>),
}

/// Follow the unit's log: the backlog, then every entry written until
/// the stream is dropped, which ends journalctl.
pub fn follow() -> impl Stream<Item = Event> + Send {
    follow_command(JOURNALCTL, arguments())
}

/// [`follow`] with any program in journalctl's place.
fn follow_command(
    program: &'static str,
    arguments: Vec<String>,
) -> impl Stream<Item = Event> + Send {
    cosmic::iced::stream::channel(16, move |output| {
        let mut command = Command::new(program);
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        read(command, output)
    })
}

/// A line journalctl wrote, and where.
enum Line {
    Out(String),
    Err(String),
}

/// Run `command` and report what it writes until it exits.
async fn read(mut command: Command, mut output: mpsc::Sender<Event>) {
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            let end = if err.kind() == io::ErrorKind::NotFound {
                End::Missing
            } else {
                End::NotStarted(err.to_string())
            };
            let _ = output.send(Event::Ended(end)).await;
            return;
        }
    };
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        unreachable!("both of journalctl's outputs are piped");
    };
    let zone = TimeZone::system();
    let mut lines = pin!(
        stream::select(lines(stdout).map(Line::Out), lines(stderr).map(Line::Err))
            .ready_chunks(BATCH)
    );
    // Whether the window has been told the log is being followed: by
    // the first entries, or by a quiet moment when there are none.
    let mut announced = false;
    loop {
        let next = if announced {
            lines.next().await
        } else {
            match tokio::time::timeout(QUIET, lines.next()).await {
                Ok(next) => next,
                Err(_) => {
                    announced = true;
                    if output.send(Event::Following).await.is_err() {
                        return;
                    }
                    continue;
                }
            }
        };
        let Some(chunk) = next else { break };
        let mut entries = Vec::with_capacity(chunk.len());
        let mut notices = Vec::new();
        for line in chunk {
            match line {
                Line::Out(line) => entries.extend(Entry::parse(&line, &zone)),
                Line::Err(line) if !line.trim().is_empty() => notices.push(Event::Notice(line)),
                Line::Err(_) => {}
            }
        }
        let mut events = Vec::with_capacity(notices.len() + 1);
        if !entries.is_empty() {
            events.push(Event::Entries(entries));
        } else if !announced {
            events.push(Event::Following);
        }
        announced = true;
        events.append(&mut notices);
        for event in events {
            if output.send(event).await.is_err() {
                return;
            }
        }
    }
    let code = child.wait().await.ok().and_then(|status| status.code());
    let _ = output.send(Event::Ended(End::Exited(code))).await;
}

/// The lines `reader` yields, until it ends. A line that is not UTF-8
/// is read with replacement characters rather than ending the stream.
fn lines(reader: impl AsyncRead + Unpin + Send) -> impl Stream<Item = String> + Send {
    stream::unfold(
        BufReader::new(reader).split(b'\n'),
        |mut lines| async move {
            let line = lines.next_segment().await.ok().flatten()?;
            Some((String::from_utf8_lossy(&line).into_owned(), lines))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An entry from xremap, as journalctl writes it.
    const XREMAP: &str = r#"{"_PID":"3320","__CURSOR":"s=4de0;i=e08426","__SEQNUM":"14713894","__MONOTONIC_TIMESTAMP":"8651395986","SYSLOG_IDENTIFIER":"xremap","__REALTIME_TIMESTAMP":"1791001325156569","MESSAGE":"application-client: COSMIC (supported: true)","PRIORITY":"6","_SYSTEMD_INVOCATION_ID":"05b151a25bb54f0bac03b5caef52c553","_BOOT_ID":"8fbdff34b15b48a890f2457c54a52ac4"}"#;

    /// systemd's user manager starting the unit.
    const STARTED: &str = r#"{"MESSAGE":"Started xremap.service - Keyboard remapping for Keyloom (xremap).","USER_INVOCATION_ID":"05b151a25bb54f0bac03b5caef52c553","PRIORITY":"6","SYSLOG_IDENTIFIER":"systemd","__REALTIME_TIMESTAMP":"1790999895000000"}"#;

    fn parse(line: &str) -> Entry {
        Entry::parse(line, &TimeZone::UTC).expect("an entry")
    }

    #[test]
    fn an_entry_from_xremap_keeps_its_time_text_and_run() {
        let entry = parse(XREMAP);

        assert_eq!(entry.clock, "04:22:05");
        assert_eq!(entry.at.date(), jiff::civil::date(2026, 10, 3));
        assert_eq!(entry.text, "application-client: COSMIC (supported: true)");
        assert_eq!(entry.writer, "xremap");
        assert_eq!(entry.source, Source::Remapper);
        assert_eq!(entry.priority, 6);
        assert_eq!(
            entry.run.as_deref(),
            Some("05b151a25bb54f0bac03b5caef52c553")
        );
    }

    #[test]
    fn systemds_messages_about_the_unit_belong_to_the_run_they_describe() {
        let entry = parse(STARTED);

        assert_eq!(entry.source, Source::Systemd);
        assert_eq!(
            entry.run.as_deref(),
            Some("05b151a25bb54f0bac03b5caef52c553")
        );
        assert_eq!(
            entry.to_line(),
            "2026-10-03 03:58:15 systemd: Started xremap.service - Keyboard remapping for \
             Keyloom (xremap)."
        );
    }

    #[test]
    fn the_time_is_shown_in_the_local_zone() {
        let zone = TimeZone::fixed(jiff::tz::offset(-5));

        let entry = Entry::parse(XREMAP, &zone).expect("an entry");

        assert_eq!(entry.clock, "23:22:05");
        assert_eq!(entry.at.date(), jiff::civil::date(2026, 10, 2));
    }

    #[test]
    fn a_message_that_is_not_utf8_is_read_with_replacements_and_trimmed() {
        let entry = parse(
            r#"{"__REALTIME_TIMESTAMP":"1790999895000000","MESSAGE":[79,75,255,10],"SYSLOG_IDENTIFIER":"xremap"}"#,
        );

        assert_eq!(entry.text, "OK\u{fffd}");
        assert_eq!(entry.priority, 6, "a missing priority reads as info");
        assert_eq!(entry.run, None);
    }

    #[test]
    fn a_field_held_more_than_once_reads_as_its_first_value() {
        let entry = parse(
            r#"{"__REALTIME_TIMESTAMP":"1790999895000000","MESSAGE":"x","SYSLOG_IDENTIFIER":["xremap","other"]}"#,
        );

        assert_eq!(entry.writer, "xremap");
    }

    #[test]
    fn lines_that_are_not_entries_are_left_out() {
        for line in [
            "",
            "-- No entries --",
            r#"{"MESSAGE":"no time"}"#,
            r#"{"__REALTIME_TIMESTAMP":"soon","MESSAGE":"x"}"#,
        ] {
            assert_eq!(Entry::parse(line, &TimeZone::UTC), None, "{line:?}");
        }
    }

    #[test]
    fn severity_comes_from_the_priority_or_xremaps_wording() {
        let mut entry = parse(XREMAP);
        assert_eq!(entry.severity(), Severity::Normal);

        entry.text = "Failed to ungrab device: No such device (os error 19)".to_owned();
        assert_eq!(entry.severity(), Severity::Warning);
        entry.text = "Error: Failed to parse config".to_owned();
        assert_eq!(entry.severity(), Severity::Error);
        entry.text = "thread 'main' panicked at src/main.rs:1:1".to_owned();
        assert_eq!(entry.severity(), Severity::Error);

        let mut entry = parse(STARTED);
        entry.text = "Failed to start xremap.service".to_owned();
        assert_eq!(
            entry.severity(),
            Severity::Normal,
            "systemd says by priority"
        );
        entry.priority = 4;
        assert_eq!(entry.severity(), Severity::Warning);
        entry.priority = 3;
        assert_eq!(entry.severity(), Severity::Error);
    }

    #[test]
    fn journalctl_follows_the_units_backlog_as_json() {
        let arguments = arguments();

        for argument in [
            "--user",
            "--unit=xremap.service",
            "--follow",
            "--lines=1000",
            "--output=json",
            "--all",
            "--no-pager",
        ] {
            assert!(
                arguments.iter().any(|given| given == argument),
                "{argument} in {arguments:?}"
            );
        }
        assert_eq!(terminal_command(), "journalctl --user -u xremap.service");
    }

    /// Everything following `script` (run by `sh`) reports.
    fn events_of(script: &str) -> Vec<Event> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime");
        runtime.block_on(
            follow_command("sh", vec!["-c".to_owned(), script.to_owned()]).collect::<Vec<_>>(),
        )
    }

    #[test]
    fn following_reports_entries_notices_and_how_it_ended() {
        let script = format!(
            "echo '{XREMAP}'; echo 'not json'; echo '{STARTED}'; echo 'a warning' >&2; exit 3"
        );

        let events = events_of(&script);

        let entries: Vec<String> = events
            .iter()
            .filter_map(|event| match event {
                Event::Entries(entries) => Some(entries.iter().map(|entry| entry.text.clone())),
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(entries.len(), 2, "{events:?}");
        assert!(entries[0].starts_with("application-client"));
        assert!(entries[1].starts_with("Started"));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, Event::Notice(notice) if notice == "a warning")),
            "{events:?}"
        );
        assert!(
            matches!(events.last(), Some(Event::Ended(End::Exited(Some(3))))),
            "{events:?}"
        );
    }

    #[test]
    fn entries_written_later_arrive_while_journalctl_runs() {
        let script = format!("echo '{XREMAP}'; sleep 0.5; echo '{STARTED}'");

        let events = events_of(&script);

        let batches: Vec<usize> = events
            .iter()
            .filter_map(|event| match event {
                Event::Entries(entries) => Some(entries.len()),
                _ => None,
            })
            .collect();
        assert_eq!(batches, [1, 1], "each as it was written: {events:?}");
    }

    #[test]
    fn a_quiet_log_is_announced_as_followed() {
        let events = events_of("sleep 0.6");

        assert!(
            matches!(
                events.as_slice(),
                [Event::Following, Event::Ended(End::Exited(Some(0)))]
            ),
            "{events:?}"
        );
    }

    #[test]
    fn a_missing_program_is_reported_as_missing() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime");

        let events = runtime
            .block_on(follow_command("keyloom-no-such-journalctl", Vec::new()).collect::<Vec<_>>());

        assert!(
            matches!(events.as_slice(), [Event::Ended(End::Missing)]),
            "{events:?}"
        );
    }
}

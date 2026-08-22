//! Global keyboard monitoring via evdev (`/dev/input/event*`).
//!
//! Reading evdev devices is non-exclusive: the compositor still receives
//! every event, we merely observe them. This is the standard way to watch
//! keyboard input system-wide on Linux, and it requires read access to
//! `/dev/input` (i.e. membership in the `input` group).

use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{Stream, StreamExt, stream};
use evdev::{Device, EventSummary, KeyCode};

/// A physical key event observed from an input device.
#[derive(Clone, Copy, Debug)]
pub enum KeyEvent {
    Pressed(u16),
    Repeated(u16),
    Released(u16),
}

/// What the monitor reports to the application.
#[derive(Clone, Debug)]
pub enum Event {
    /// Monitoring started on this many keyboard devices.
    Started { devices: usize },
    /// A key event was observed on some keyboard.
    Key(KeyEvent),
}

/// Whether a device looks like a real keyboard (reports letter keys).
fn is_keyboard(device: &Device) -> bool {
    device.supported_keys().is_some_and(|keys| {
        keys.contains(KeyCode::KEY_A) && keys.contains(KeyCode::KEY_SPACE)
    })
}

/// Stream of [`Event`]s from every keyboard-capable evdev device.
///
/// Each device is read on its own blocking thread; events are forwarded
/// through a channel that this stream yields from. Threads exit when the
/// stream (and thus the receiver) is dropped or the device goes away.
pub fn watch() -> impl Stream<Item = Event> + Send {
    let (tx, rx) = mpsc::unbounded::<Event>();

    let keyboards: Vec<(std::path::PathBuf, Device)> = evdev::enumerate()
        .filter(|(_, device)| is_keyboard(device))
        .collect();

    let started = Event::Started {
        devices: keyboards.len(),
    };

    for (path, mut device) in keyboards {
        let tx = tx.clone();

        std::thread::spawn(move || {
            eprintln!(
                "monitoring {} ({})",
                path.display(),
                device.name().unwrap_or("unnamed")
            );

            loop {
                let events = match device.fetch_events() {
                    Ok(events) => events,
                    Err(err) => {
                        eprintln!("stopped monitoring {}: {err}", path.display());
                        return;
                    }
                };

                for event in events {
                    let EventSummary::Key(_, code, value) = event.destructure() else {
                        continue;
                    };

                    let key_event = match value {
                        0 => KeyEvent::Released(code.0),
                        1 => KeyEvent::Pressed(code.0),
                        2 => KeyEvent::Repeated(code.0),
                        _ => continue,
                    };

                    // Receiver dropped: subscription ended, stop the thread.
                    if tx.unbounded_send(Event::Key(key_event)).is_err() {
                        return;
                    }
                }
            }
        });
    }

    // Prepend the startup notice so the UI can report the device count
    // (or the lack of access to any device).
    stream::iter([started]).chain(rx)
}

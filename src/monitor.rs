//! Global keyboard monitoring via evdev (`/dev/input/event*`).
//!
//! Reading evdev devices is non-exclusive: the compositor still receives
//! every event, we merely observe them. This is the standard way to watch
//! keyboard input system-wide on Linux, and it requires read access to
//! `/dev/input` (i.e. membership in the `input` group).

use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{Stream, StreamExt, stream};
use evdev::{Device, EventSummary, KeyCode};
use std::path::PathBuf;

use crate::keyboard;

/// A physical key event observed from an input device.
#[derive(Clone, Copy, Debug)]
pub enum KeyEvent {
    Pressed(u16),
    Repeated(#[allow(dead_code)] u16),
    Released(u16),
}

/// A readable keyboard, identified by its evdev path rather than its name.
#[derive(Clone, Debug)]
pub struct KeyboardDevice {
    pub path: PathBuf,
    pub name: String,
    pub connected: bool,
    /// Best-effort form factor index and whether the name contributed.
    /// Currently unused by the UI; kept for the remapping engine work.
    #[allow(dead_code)]
    pub form: usize,
    #[allow(dead_code)]
    pub form_hinted: bool,
}

/// Prefer size hints in device names over potentially inflated capabilities.
#[allow(dead_code)]
pub fn detected_form<'a>(devices: impl Iterator<Item = &'a KeyboardDevice>) -> Option<usize> {
    let mut hinted = None;
    let mut fallback = None;
    for device in devices.filter(|device| device.connected) {
        fallback = Some(fallback.map_or(device.form, |form: usize| form.min(device.form)));
        if device.form_hinted {
            hinted = Some(hinted.map_or(device.form, |form: usize| form.min(device.form)));
        }
    }
    hinted.or(fallback)
}

/// What the monitor reports to the application.
#[derive(Clone, Debug)]
pub enum Event {
    /// Readable keyboards discovered at startup, in display order.
    Started(Vec<KeyboardDevice>),
    /// A key event and the keyboard that produced it.
    Key { device: PathBuf, event: KeyEvent },
    /// A keyboard could no longer be read.
    Disconnected(PathBuf),
}

/// Whether a device looks like a real keyboard (reports letter keys).
fn is_keyboard(device: &Device) -> bool {
    device
        .supported_keys()
        .is_some_and(|keys| keys.contains(KeyCode::KEY_A) && keys.contains(KeyCode::KEY_SPACE))
}

/// Guess a device's form factor from its name and reported keys (see
/// [`keyboard::form_for_keys`] and [`keyboard::form_for_name`] for the
/// caveats). Returns whether the name contributed, and the guess.
fn form_guess(device: &Device) -> (bool, usize) {
    let has = |key: KeyCode| {
        device
            .supported_keys()
            .is_some_and(|keys| keys.contains(key))
    };

    let caps = keyboard::form_for_keys(
        has(KeyCode::KEY_KP0),
        has(KeyCode::KEY_SCROLLLOCK) && has(KeyCode::KEY_PAUSE),
        has(KeyCode::KEY_F1),
        has(KeyCode::KEY_UP),
    );

    // A name hint can only shrink the capability guess: keys a device
    // doesn't report are conclusively absent, over-reported ones are not.
    match keyboard::form_for_name(device.name().unwrap_or("")) {
        Some(name) => (true, name.max(caps)),
        None => (false, caps),
    }
}

/// Stream of [`Event`]s from every keyboard-capable evdev device.
///
/// Each device is read on its own blocking thread; events are forwarded
/// through a channel that this stream yields from. Threads exit when the
/// stream (and thus the receiver) is dropped or the device goes away.
pub fn watch() -> impl Stream<Item = Event> + Send {
    let (tx, rx) = mpsc::unbounded::<Event>();

    let mut keyboards: Vec<(PathBuf, Device)> = evdev::enumerate()
        .filter(|(_, device)| is_keyboard(device))
        .collect();
    keyboards.sort_by(|(path_a, a), (path_b, b)| {
        a.name().cmp(&b.name()).then_with(|| path_a.cmp(path_b))
    });

    let devices = keyboards
        .iter()
        .map(|(path, device)| {
            let (form_hinted, form) = form_guess(device);
            KeyboardDevice {
                path: path.clone(),
                name: device
                    .name()
                    .filter(|name| !name.is_empty())
                    .unwrap_or("Unnamed keyboard")
                    .to_owned(),
                connected: true,
                form,
                form_hinted,
            }
        })
        .collect();
    let started = Event::Started(devices);

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
                        let _ = tx.unbounded_send(Event::Disconnected(path.clone()));
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
                    if tx
                        .unbounded_send(Event::Key {
                            device: path.clone(),
                            event: key_event,
                        })
                        .is_err()
                    {
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

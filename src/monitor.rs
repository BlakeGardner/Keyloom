//! Global keyboard monitoring via evdev (`/dev/input/event*`).
//!
//! Reading evdev devices is non-exclusive: the compositor still receives
//! every event, we merely observe them. This is the standard way to watch
//! keyboard input system-wide on Linux, and it requires read access to
//! `/dev/input` (i.e. membership in the `input` group).

use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{Stream, StreamExt, stream};
use evdev::{Device, EventSummary, KeyCode};

use crate::keyboard;

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
    Started {
        devices: usize,
        /// Best-effort form factor guess (an index into
        /// [`keyboard::FORM_FACTORS`]) from the keys the devices report.
        form: Option<usize>,
    },
    /// A key event was observed on some keyboard.
    Key(KeyEvent),
}

/// Whether a device looks like a real keyboard (reports letter keys).
fn is_keyboard(device: &Device) -> bool {
    device.supported_keys().is_some_and(|keys| {
        keys.contains(KeyCode::KEY_A) && keys.contains(KeyCode::KEY_SPACE)
    })
}

/// Guess a device's form factor from its name and reported keys (see
/// [`keyboard::form_for_keys`] and [`keyboard::form_for_name`] for the
/// caveats). Returns whether the name contributed, and the guess.
fn form_guess(device: &Device) -> (bool, usize) {
    let has = |key: KeyCode| device.supported_keys().is_some_and(|keys| keys.contains(key));

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

    let keyboards: Vec<(std::path::PathBuf, Device)> = evdev::enumerate()
        .filter(|(_, device)| is_keyboard(device))
        .collect();

    // Devices whose names reveal their size are the most trustworthy
    // (KVMs and remappers emulate full-size boards); within the preferred
    // group, the largest (lowest-index) guess wins so every physical key
    // is still represented.
    let guesses: Vec<(bool, usize)> = keyboards
        .iter()
        .map(|(_, device)| form_guess(device))
        .collect();

    let form = guesses
        .iter()
        .filter_map(|&(hinted, guess)| hinted.then_some(guess))
        .min()
        .or_else(|| guesses.iter().map(|&(_, guess)| guess).min());

    if let Some(index) = form {
        eprintln!(
            "physical keyboards suggest a {} board",
            keyboard::FORM_FACTORS[index].name
        );
    }

    let started = Event::Started {
        devices: keyboards.len(),
        form,
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

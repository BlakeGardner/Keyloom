//! Global keyboard monitoring via evdev (`/dev/input/event*`).
//!
//! Reading evdev devices is non-exclusive: the compositor still receives
//! every event, we merely observe them. This is the standard way to watch
//! keyboard input system-wide on Linux, and it requires read access to
//! `/dev/input` (i.e. membership in the `input` group).

use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{Stream, StreamExt, stream};
use evdev::{Device, EventSummary, KeyCode};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::keyboard;

/// How often to look for newly attached keyboards.
const HOTPLUG_INTERVAL: Duration = Duration::from_secs(2);

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
    pub form: usize,
    pub form_hinted: bool,
}

/// Prefer size hints in device names over potentially inflated capabilities.
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
    /// A keyboard attached (or re-attached) after startup.
    Connected(KeyboardDevice),
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

/// Describe one opened device for the application.
fn device_entry(path: &Path, device: &Device) -> KeyboardDevice {
    let (form_hinted, form) = form_guess(device);
    KeyboardDevice {
        path: path.to_owned(),
        name: device
            .name()
            .filter(|name| !name.is_empty())
            .unwrap_or("Unnamed keyboard")
            .to_owned(),
        connected: true,
        form,
        form_hinted,
    }
}

/// The `/dev/input/event*` nodes currently present, whether readable
/// or not.
fn input_nodes() -> HashSet<PathBuf> {
    std::fs::read_dir("/dev/input")
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("event"))
        })
        .collect()
}

/// Read one device on a blocking thread, forwarding its key events.
/// The thread exits when the receiver is dropped or the device goes
/// away (reported as [`Event::Disconnected`]).
fn spawn_reader(path: PathBuf, mut device: Device, tx: mpsc::UnboundedSender<Event>) {
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

/// Stream of [`Event`]s from every keyboard-capable evdev device.
///
/// Each device is read on its own blocking thread; events are forwarded
/// through a channel that this stream yields from. Threads exit when the
/// stream (and thus the receiver) is dropped or the device goes away.
/// A scanner thread polls `/dev/input` so keyboards plugged in after
/// launch are picked up too.
pub fn watch() -> impl Stream<Item = Event> + Send {
    let (tx, rx) = mpsc::unbounded::<Event>();

    // Snapshot the node list before enumerating: a keyboard appearing
    // in between is then unknown and adopted by the scanner instead of
    // being missed.
    let mut known = input_nodes();

    let mut keyboards: Vec<(PathBuf, Device)> = evdev::enumerate()
        .filter(|(path, device)| known.contains(path) && is_keyboard(device))
        .collect();
    keyboards.sort_by(|(path_a, a), (path_b, b)| {
        a.name().cmp(&b.name()).then_with(|| path_a.cmp(path_b))
    });

    let devices = keyboards
        .iter()
        .map(|(path, device)| device_entry(path, device))
        .collect();
    let started = Event::Started(devices);

    // Everything in the snapshot is now known: keyboards are monitored
    // below, the rest was rejected (or unreadable) during enumeration.

    for (path, device) in keyboards {
        spawn_reader(path, device, tx.clone());
    }

    // Hotplug scanner: watch for new event nodes and adopt the ones
    // that turn out to be keyboards.
    let scan_tx = tx.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(HOTPLUG_INTERVAL);
            if scan_tx.is_closed() {
                return;
            }

            let current = input_nodes();
            // Forget nodes that disappeared so a keyboard replugged
            // onto the same node is picked up again.
            known.retain(|path| current.contains(path));

            for path in current {
                if known.contains(&path) {
                    continue;
                }
                // Right after attach the node may not be readable yet
                // (udev still applying permissions); retry next scan.
                let Ok(device) = Device::open(&path) else {
                    continue;
                };
                known.insert(path.clone());
                if !is_keyboard(&device) {
                    continue;
                }
                if scan_tx
                    .unbounded_send(Event::Connected(device_entry(&path, &device)))
                    .is_err()
                {
                    return;
                }
                spawn_reader(path, device, scan_tx.clone());
            }
        }
    });

    // Prepend the startup notice so the UI can report the device count
    // (or the lack of access to any device).
    stream::iter([started]).chain(rx)
}

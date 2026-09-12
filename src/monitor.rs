//! Global keyboard monitoring via evdev (`/dev/input/event*`).
//!
//! Reading evdev devices is non-exclusive: the compositor still receives
//! every event, we merely observe them. This is the standard way to watch
//! keyboard input system-wide on Linux, and it requires read access to
//! `/dev/input` (i.e. membership in the `input` group).

use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{Stream, StreamExt, stream};
use evdev::{Device, EventSummary, InputId, KeyCode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
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
    /// Identity used to restore display preferences across event-node changes.
    pub id: KeyboardId,
    pub name: String,
    pub connected: bool,
    /// Best-effort form factor index and whether the name contributed.
    pub form: usize,
    pub form_hinted: bool,
    /// Whether the device reports the physical ISO 102nd key.
    pub iso: bool,
    /// Whether Linux exposes this as a software-created input device.
    pub virtual_device: bool,
}

/// Best available persistent identity, independent of `/dev/input/eventN`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyboardId {
    bus: u16,
    vendor: u16,
    product: u16,
    location: KeyboardLocation,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum KeyboardLocation {
    Unique(String),
    Physical(String),
    Name(String),
}

impl KeyboardId {
    /// Prefer a serial/unique identifier, then a connection path. Devices
    /// lacking both can only be distinguished by their model and name.
    pub fn new(input: InputId, unique: Option<&str>, physical: Option<&str>, name: &str) -> Self {
        let location = if let Some(unique) = unique.filter(|value| !value.is_empty()) {
            KeyboardLocation::Unique(unique.to_owned())
        } else if let Some(physical) = physical.filter(|value| !value.is_empty()) {
            KeyboardLocation::Physical(physical.to_owned())
        } else {
            KeyboardLocation::Name(name.to_owned())
        };
        Self {
            bus: input.bus_type().0,
            vendor: input.vendor(),
            product: input.product(),
            location,
        }
    }
}

/// Prefer size hints in device names over potentially inflated capabilities.
pub fn detected_form<'a>(devices: impl Iterator<Item = &'a KeyboardDevice>) -> Option<usize> {
    let mut physical_hinted = None;
    let mut physical_fallback = None;
    let mut virtual_hinted = None;
    let mut virtual_fallback = None;
    for device in devices.filter(|device| device.connected) {
        let (hinted, fallback) = if device.virtual_device {
            (&mut virtual_hinted, &mut virtual_fallback)
        } else {
            (&mut physical_hinted, &mut physical_fallback)
        };
        *fallback = Some(fallback.map_or(device.form, |form: usize| form.min(device.form)));
        if device.form_hinted {
            *hinted = Some(hinted.map_or(device.form, |form: usize| form.min(device.form)));
        }
    }
    physical_hinted
        .or(physical_fallback)
        .or(virtual_hinted)
        .or(virtual_fallback)
}

/// Use the ISO assembly when any connected keyboard reports its extra key.
pub fn detected_iso<'a>(devices: impl Iterator<Item = &'a KeyboardDevice>) -> Option<bool> {
    let mut physical_connected = false;
    let mut physical_iso = false;
    let mut virtual_connected = false;
    let mut virtual_iso = false;
    for device in devices.filter(|device| device.connected) {
        if device.virtual_device {
            virtual_connected = true;
            virtual_iso |= device.iso;
        } else {
            physical_connected = true;
            physical_iso |= device.iso;
        }
    }
    physical_connected
        .then_some(physical_iso)
        .or_else(|| virtual_connected.then_some(virtual_iso))
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

/// Linux places uinput and other software-created devices under this sysfs
/// subtree, regardless of the desktop environment running above it.
fn is_virtual_device(path: &Path) -> bool {
    let Some(event) = path.file_name() else {
        return false;
    };
    std::fs::canonicalize(Path::new("/sys/class/input").join(event).join("device"))
        .is_ok_and(|path| path.starts_with("/sys/devices/virtual/input"))
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
    let name = device
        .name()
        .filter(|name| !name.is_empty())
        .unwrap_or("Unnamed keyboard");
    KeyboardDevice {
        path: path.to_owned(),
        id: KeyboardId::new(
            device.input_id(),
            device.unique_name(),
            device.physical_path(),
            name,
        ),
        name: name.to_owned(),
        connected: true,
        form,
        form_hinted,
        iso: device
            .supported_keys()
            .is_some_and(|keys| keys.contains(KeyCode::KEY_102ND)),
        virtual_device: is_virtual_device(path),
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

/// Keep a keyboard's path reserved until its reader has sent its final
/// events and exited. Other input nodes only need to be classified once.
enum WatchedNode {
    Keyboard(JoinHandle<()>),
    Other,
}

/// Find nodes that need opening, including keyboards whose readers stopped.
fn scan_candidates(
    known: &mut HashMap<PathBuf, WatchedNode>,
    current: HashSet<PathBuf>,
) -> Vec<PathBuf> {
    known.retain(|path, node| match node {
        // xremap can recreate its virtual keyboard at the same path between
        // scans. Path presence alone does not mean we still have a reader.
        // Conversely, don't replace a reader until its Disconnected event
        // has been queued, even if its path temporarily disappears.
        WatchedNode::Keyboard(reader) => !reader.is_finished(),
        WatchedNode::Other => current.contains(path),
    });
    current
        .into_iter()
        .filter(|path| !known.contains_key(path))
        .collect()
}

/// Read one device on a blocking thread, forwarding its key events.
/// The thread exits when the receiver is dropped or the device goes
/// away (reported as [`Event::Disconnected`]).
fn spawn_reader(
    path: PathBuf,
    mut device: Device,
    tx: mpsc::UnboundedSender<Event>,
) -> JoinHandle<()> {
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
    })
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

    // Only remember devices we could open. Nodes missed by enumeration
    // (including ones awaiting udev permissions) are retried by the scanner.
    let mut known = HashMap::new();
    let mut keyboards = Vec::new();
    for (path, device) in evdev::enumerate() {
        if is_keyboard(&device) {
            keyboards.push((path, device));
        } else {
            known.insert(path, WatchedNode::Other);
        }
    }
    keyboards.sort_by(|(path_a, a), (path_b, b)| {
        a.name().cmp(&b.name()).then_with(|| path_a.cmp(path_b))
    });

    let devices = keyboards
        .iter()
        .map(|(path, device)| device_entry(path, device))
        .collect();
    let started = Event::Started(devices);

    for (path, device) in keyboards {
        let reader = spawn_reader(path.clone(), device, tx.clone());
        known.insert(path, WatchedNode::Keyboard(reader));
    }

    // Hotplug scanner: adopt new keyboards and replace stopped readers,
    // including when a virtual device is recreated at the same event node.
    let scan_tx = tx.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(HOTPLUG_INTERVAL);
            if scan_tx.is_closed() {
                return;
            }

            for path in scan_candidates(&mut known, input_nodes()) {
                // Right after attach the node may not be readable yet
                // (udev still applying permissions); retry next scan.
                let Ok(device) = Device::open(&path) else {
                    continue;
                };
                if !is_keyboard(&device) {
                    known.insert(path, WatchedNode::Other);
                    continue;
                }
                if scan_tx
                    .unbounded_send(Event::Connected(device_entry(&path, &device)))
                    .is_err()
                {
                    return;
                }
                let reader = spawn_reader(path.clone(), device, scan_tx.clone());
                known.insert(path, WatchedNode::Keyboard(reader));
            }
        }
    });

    // Prepend the startup notice so the UI can report the device count
    // (or the lack of access to any device).
    stream::iter([started]).chain(rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc as sync_mpsc;
    use std::time::Instant;

    fn layout_device(connected: bool, iso: bool, virtual_device: bool) -> KeyboardDevice {
        KeyboardDevice {
            path: PathBuf::from("/dev/input/event0"),
            id: KeyboardId::new(
                InputId::new(evdev::BusType::BUS_USB, 1, 2, 1),
                Some("serial"),
                None,
                "Keyboard",
            ),
            name: "Keyboard".to_owned(),
            connected,
            form: keyboard::FORM_TKL,
            form_hinted: true,
            iso,
            virtual_device,
        }
    }

    #[test]
    fn aggregate_variant_uses_connected_keyboard_capabilities() {
        assert_eq!(detected_iso(std::iter::empty()), None);
        let ansi = layout_device(true, false, false);
        let iso = layout_device(true, true, false);
        let disconnected_iso = layout_device(false, true, false);
        assert_eq!(detected_iso([&ansi].into_iter()), Some(false));
        assert_eq!(detected_iso([&ansi, &iso].into_iter()), Some(true));
        assert_eq!(
            detected_iso([&ansi, &disconnected_iso].into_iter()),
            Some(false)
        );
        let broad_virtual = layout_device(true, true, true);
        assert_eq!(
            detected_iso([&ansi, &broad_virtual].into_iter()),
            Some(false),
            "a virtual keyboard does not distort a physical keyboard's variant"
        );
        assert_eq!(
            detected_iso([&broad_virtual].into_iter()),
            Some(true),
            "virtual keyboards remain a fallback when no physical keyboard is visible"
        );
    }

    #[test]
    fn aggregate_form_prefers_physical_keyboards_over_virtual_capabilities() {
        let physical = layout_device(true, false, false);
        let mut broad_virtual = layout_device(true, true, true);
        broad_virtual.form = keyboard::FORM_FULL;
        assert_eq!(
            detected_form([&physical, &broad_virtual].into_iter()),
            Some(keyboard::FORM_TKL)
        );
        assert_eq!(
            detected_form([&broad_virtual].into_iter()),
            Some(keyboard::FORM_FULL)
        );
    }

    #[test]
    fn unique_identity_survives_port_and_firmware_changes_and_distinguishes_units() {
        let input = InputId::new(evdev::BusType::BUS_USB, 1, 2, 1);
        let original = KeyboardId::new(input.clone(), Some("serial-a"), Some("usb-1"), "Keyboard");
        let moved = KeyboardId::new(
            InputId::new(evdev::BusType::BUS_USB, 1, 2, 2),
            Some("serial-a"),
            Some("usb-2"),
            "Keyboard",
        );
        let other = KeyboardId::new(input, Some("serial-b"), Some("usb-1"), "Keyboard");
        assert_eq!(original, moved);
        assert_ne!(original, other);
    }

    #[test]
    fn fallback_identity_uses_connection_then_model_and_name() {
        let input = InputId::new(evdev::BusType::BUS_USB, 1, 2, 1);
        let first = KeyboardId::new(input.clone(), None, Some("usb-1"), "Keyboard");
        assert_eq!(
            first,
            KeyboardId::new(input.clone(), Some(""), Some("usb-1"), "Keyboard")
        );
        assert_ne!(
            first,
            KeyboardId::new(input.clone(), None, Some("usb-2"), "Keyboard")
        );
        let fallback = KeyboardId::new(input.clone(), None, None, "Keyboard");
        assert_eq!(
            fallback,
            KeyboardId::new(input.clone(), Some(""), Some(""), "Keyboard")
        );
        assert_ne!(
            fallback,
            KeyboardId::new(input, None, None, "Other keyboard")
        );
        assert_ne!(
            fallback,
            KeyboardId::new(
                InputId::new(evdev::BusType::BUS_USB, 1, 3, 1),
                None,
                None,
                "Keyboard"
            )
        );
    }

    /// Stand in for a reader blocked waiting for device events.
    fn running_reader() -> (WatchedNode, sync_mpsc::Sender<()>) {
        let (tx, rx) = sync_mpsc::channel();
        let reader = std::thread::spawn(move || {
            let _ = rx.recv();
        });
        (WatchedNode::Keyboard(reader), tx)
    }

    fn wait_for_reader(node: &WatchedNode) {
        let WatchedNode::Keyboard(reader) = node else {
            panic!("expected a keyboard reader");
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while !reader.is_finished() {
            assert!(Instant::now() < deadline, "reader did not exit");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn stopped_reader_is_retried_when_the_node_never_disappears_from_scans() {
        let remapper = PathBuf::from("/dev/input/event19");
        let physical = PathBuf::from("/dev/input/event4");
        let current = HashSet::from([remapper.clone(), physical.clone()]);
        let (reader, mut stop) = running_reader();
        let (physical_reader, _physical_stop) = running_reader();
        let mut known = HashMap::from([(remapper.clone(), reader), (physical, physical_reader)]);

        // Repeated applies can replace xremap's device between scans without
        // changing the node list. Each stopped reader must be replaced once.
        for _ in 0..2 {
            assert!(scan_candidates(&mut known, current.clone()).is_empty());
            stop.send(()).unwrap();
            wait_for_reader(&known[&remapper]);
            assert_eq!(
                scan_candidates(&mut known, current.clone()),
                vec![remapper.clone()]
            );

            let (replacement, replacement_stop) = running_reader();
            known.insert(remapper.clone(), replacement);
            stop = replacement_stop;
            assert!(scan_candidates(&mut known, current.clone()).is_empty());
        }
    }

    #[test]
    fn disappearing_node_waits_for_its_reader_to_finish_before_reconnecting() {
        let path = PathBuf::from("/dev/input/event19");
        let current = HashSet::from([path.clone()]);
        let (reader, stop) = running_reader();
        let mut known = HashMap::from([(path.clone(), reader)]);

        // The old reader may still have events to send, including its final
        // Disconnected. Don't let those arrive after a new Connected notice.
        assert!(scan_candidates(&mut known, HashSet::new()).is_empty());
        assert!(scan_candidates(&mut known, current.clone()).is_empty());
        stop.send(()).unwrap();
        wait_for_reader(&known[&path]);
        assert_eq!(scan_candidates(&mut known, current), vec![path]);
    }

    #[test]
    fn unreadable_nodes_are_retried_while_known_other_devices_are_skipped() {
        let keyboard = PathBuf::from("/dev/input/event19");
        let mouse = PathBuf::from("/dev/input/event1");
        let current = HashSet::from([keyboard.clone(), mouse.clone()]);
        let mut known = HashMap::from([(mouse.clone(), WatchedNode::Other)]);

        // A failed open leaves the node unknown so later udev permissions
        // can make it readable, including after startup enumeration.
        for _ in 0..2 {
            assert_eq!(
                scan_candidates(&mut known, current.clone()),
                vec![keyboard.clone()]
            );
        }
        assert!(scan_candidates(&mut known, HashSet::new()).is_empty());
        assert_eq!(
            scan_candidates(&mut known, HashSet::from([mouse.clone()])),
            vec![mouse]
        );
    }
}

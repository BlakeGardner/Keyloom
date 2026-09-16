//! Keyloom's generated layer and application configurations, run
//! through xremap's own event handler. This file is compiled inside a
//! checkout of xremap by `scripts/verify-layers-with-xremap.sh`, which
//! also dumps the documents it reads from `keyloom/` next to it.

use crate::action::Action;
use crate::event::{Event, KeyEvent, KeyValue};
use crate::tests::{assert_actions, assert_actions_with_current_application};
use evdev::KeyCode as Key;
use std::time::Duration;

const NAVIGATION: &str = include_str!("keyloom/navigation.yml");
const TWO_LAYERS: &str = include_str!("keyloom/two-layers.yml");
const REMAPPED_KEY: &str = include_str!("keyloom/remapped-key.yml");
const APP_SCOPED: &str = include_str!("keyloom/app-scoped.yml");

fn press(key: Key) -> Action {
    Action::KeyEvent(KeyEvent::new(key, KeyValue::Press))
}
fn release(key: Key) -> Action {
    Action::KeyEvent(KeyEvent::new(key, KeyValue::Release))
}
fn delay() -> Action {
    Action::Delay(Duration::from_nanos(0))
}

/// Holding Caps Lock turns H into Left; releasing Caps Lock restores H.
#[test]
fn keyloom_nav_layer_enters_and_leaves() {
    assert_actions(
        NAVIGATION,
        vec![
            Event::key_press(Key::KEY_CAPSLOCK),
            Event::key_press(Key::KEY_H),
            Event::key_release(Key::KEY_H),
            Event::key_release(Key::KEY_CAPSLOCK),
            Event::key_press(Key::KEY_H),
            Event::key_release(Key::KEY_H),
        ],
        vec![
            press(Key::KEY_LEFT),
            release(Key::KEY_LEFT),
            delay(),
            delay(),
            release(Key::KEY_H),
            press(Key::KEY_H),
            release(Key::KEY_H),
        ],
    );
}

/// Every job of the starter layer produces its key.
#[test]
fn keyloom_nav_layer_covers_every_job() {
    for (job, out) in [
        (Key::KEY_H, Key::KEY_LEFT),
        (Key::KEY_J, Key::KEY_DOWN),
        (Key::KEY_K, Key::KEY_UP),
        (Key::KEY_L, Key::KEY_RIGHT),
        (Key::KEY_U, Key::KEY_PAGEUP),
        (Key::KEY_D, Key::KEY_PAGEDOWN),
        (Key::KEY_A, Key::KEY_HOME),
        (Key::KEY_E, Key::KEY_END),
        (Key::KEY_Y, Key::KEY_BACKSPACE),
        (Key::KEY_N, Key::KEY_DELETE),
    ] {
        assert_actions(
            NAVIGATION,
            vec![
                Event::key_press(Key::KEY_CAPSLOCK),
                Event::key_press(job),
                Event::key_release(job),
                Event::key_release(Key::KEY_CAPSLOCK),
            ],
            vec![press(out), release(out), delay(), delay(), release(job)],
        );
    }
}

/// Tapping the layer key still gives its tap action (Escape).
#[test]
fn keyloom_nav_layer_key_taps_escape() {
    assert_actions(
        NAVIGATION,
        vec![
            Event::key_press(Key::KEY_CAPSLOCK),
            Event::key_release(Key::KEY_CAPSLOCK),
        ],
        vec![press(Key::KEY_ESC), release(Key::KEY_ESC)],
    );
}

/// A held Shift stays held around the layer's output: Shift+Left.
#[test]
fn keyloom_nav_layer_keeps_a_held_shift() {
    assert_actions(
        NAVIGATION,
        vec![
            Event::key_press(Key::KEY_LEFTSHIFT),
            Event::key_press(Key::KEY_CAPSLOCK),
            Event::key_press(Key::KEY_H),
            Event::key_release(Key::KEY_H),
            Event::key_release(Key::KEY_CAPSLOCK),
            Event::key_release(Key::KEY_LEFTSHIFT),
        ],
        vec![
            press(Key::KEY_LEFTSHIFT),
            press(Key::KEY_LEFT),
            release(Key::KEY_LEFT),
            delay(),
            delay(),
            release(Key::KEY_H),
            release(Key::KEY_LEFTSHIFT),
        ],
    );
}

/// Keys without a job keep working normally inside the layer.
#[test]
fn keyloom_nav_layer_passes_other_keys_through() {
    assert_actions(
        NAVIGATION,
        vec![
            Event::key_press(Key::KEY_CAPSLOCK),
            Event::key_press(Key::KEY_Q),
            Event::key_release(Key::KEY_Q),
            Event::key_release(Key::KEY_CAPSLOCK),
        ],
        vec![press(Key::KEY_Q), release(Key::KEY_Q)],
    );
}

/// A layer key without a tap action does nothing when tapped, and a
/// second layer on Space keeps Space typing a space when tapped.
#[test]
fn keyloom_two_layers() {
    // Caps Lock: plain layer key.
    assert_actions(
        TWO_LAYERS,
        vec![
            Event::key_press(Key::KEY_CAPSLOCK),
            Event::key_release(Key::KEY_CAPSLOCK),
        ],
        vec![],
    );
    assert_actions(
        TWO_LAYERS,
        vec![
            Event::key_press(Key::KEY_CAPSLOCK),
            Event::key_press(Key::KEY_H),
            Event::key_release(Key::KEY_H),
            Event::key_release(Key::KEY_CAPSLOCK),
        ],
        vec![
            press(Key::KEY_LEFT),
            release(Key::KEY_LEFT),
            delay(),
            delay(),
            release(Key::KEY_H),
        ],
    );
    // A disabled job swallows the key press.
    assert_actions(
        TWO_LAYERS,
        vec![
            Event::key_press(Key::KEY_CAPSLOCK),
            Event::key_press(Key::KEY_Y),
            Event::key_release(Key::KEY_Y),
            Event::key_release(Key::KEY_CAPSLOCK),
        ],
        vec![release(Key::KEY_Y)],
    );
    // Space: tap types a space, hold gives the Numbers layer.
    assert_actions(
        TWO_LAYERS,
        vec![
            Event::key_press(Key::KEY_SPACE),
            Event::key_release(Key::KEY_SPACE),
        ],
        vec![press(Key::KEY_SPACE), release(Key::KEY_SPACE)],
    );
    assert_actions(
        TWO_LAYERS,
        vec![
            Event::key_press(Key::KEY_SPACE),
            Event::key_press(Key::KEY_J),
            Event::key_release(Key::KEY_J),
            Event::key_press(Key::KEY_H),
            Event::key_release(Key::KEY_H),
            Event::key_release(Key::KEY_SPACE),
        ],
        vec![
            press(Key::KEY_1),
            release(Key::KEY_1),
            delay(),
            delay(),
            release(Key::KEY_J),
            press(Key::KEY_2),
            release(Key::KEY_2),
            delay(),
            delay(),
            release(Key::KEY_H),
        ],
    );
}

/// A job on a key whose normal press is remapped (H types J) still
/// fires, and the key still types J outside the layer.
#[test]
fn keyloom_layer_job_on_a_remapped_key() {
    assert_actions(
        REMAPPED_KEY,
        vec![
            Event::key_press(Key::KEY_H),
            Event::key_release(Key::KEY_H),
            Event::key_press(Key::KEY_CAPSLOCK),
            Event::key_press(Key::KEY_H),
            Event::key_release(Key::KEY_H),
            Event::key_release(Key::KEY_CAPSLOCK),
        ],
        vec![
            press(Key::KEY_J),
            release(Key::KEY_J),
            press(Key::KEY_LEFT),
            release(Key::KEY_LEFT),
            delay(),
            delay(),
            release(Key::KEY_J),
        ],
    );
}

/// The application scope of the app-scoped document, as the compositor
/// reports it.
fn terminal() -> Option<String> {
    Some(String::from("com.system76.CosmicTerm"))
}

/// A key remapped in one application only: A types B in the terminal
/// and stays A everywhere else, other applications included.
#[test]
fn keyloom_app_scope_applies_in_its_application_only() {
    let typed = vec![Event::key_press(Key::KEY_A), Event::key_release(Key::KEY_A)];
    assert_actions_with_current_application(
        APP_SCOPED,
        terminal(),
        typed.clone(),
        vec![press(Key::KEY_B), release(Key::KEY_B)],
    );
    assert_actions(
        APP_SCOPED,
        typed.clone(),
        vec![press(Key::KEY_A), release(Key::KEY_A)],
    );
    assert_actions_with_current_application(
        APP_SCOPED,
        Some(String::from("firefox")),
        typed,
        vec![press(Key::KEY_A), release(Key::KEY_A)],
    );
}

/// Caps Lock, kept normal in the terminal, is plain Caps Lock there;
/// its Escape tap and its navigation layer work everywhere else.
#[test]
fn keyloom_normal_key_switches_the_layer_off_in_its_application() {
    let held = vec![
        Event::key_press(Key::KEY_CAPSLOCK),
        Event::key_press(Key::KEY_H),
        Event::key_release(Key::KEY_H),
        Event::key_release(Key::KEY_CAPSLOCK),
    ];
    assert_actions_with_current_application(
        APP_SCOPED,
        terminal(),
        held.clone(),
        vec![
            press(Key::KEY_CAPSLOCK),
            press(Key::KEY_H),
            release(Key::KEY_H),
            release(Key::KEY_CAPSLOCK),
        ],
    );
    assert_actions(
        APP_SCOPED,
        held,
        vec![
            press(Key::KEY_LEFT),
            release(Key::KEY_LEFT),
            delay(),
            delay(),
            release(Key::KEY_H),
        ],
    );

    let tap = vec![
        Event::key_press(Key::KEY_CAPSLOCK),
        Event::key_release(Key::KEY_CAPSLOCK),
    ];
    assert_actions_with_current_application(
        APP_SCOPED,
        terminal(),
        tap.clone(),
        vec![press(Key::KEY_CAPSLOCK), release(Key::KEY_CAPSLOCK)],
    );
    assert_actions(APP_SCOPED, tap, vec![press(Key::KEY_ESC), release(Key::KEY_ESC)]);
}

/// Super+C copies the terminal's way there (Ctrl+Shift+C) and the usual
/// way elsewhere (Ctrl+C); xremap releases the held Super around the
/// output either way.
#[test]
fn keyloom_shortcut_differs_between_an_application_and_elsewhere() {
    let chord = vec![
        Event::key_press(Key::KEY_LEFTMETA),
        Event::key_press(Key::KEY_C),
        Event::key_release(Key::KEY_C),
        Event::key_release(Key::KEY_LEFTMETA),
    ];
    assert_actions_with_current_application(
        APP_SCOPED,
        terminal(),
        chord.clone(),
        vec![
            press(Key::KEY_LEFTMETA),
            press(Key::KEY_LEFTCTRL),
            press(Key::KEY_LEFTSHIFT),
            release(Key::KEY_LEFTMETA),
            press(Key::KEY_C),
            release(Key::KEY_C),
            delay(),
            press(Key::KEY_LEFTMETA),
            delay(),
            release(Key::KEY_LEFTCTRL),
            release(Key::KEY_LEFTSHIFT),
            release(Key::KEY_C),
            release(Key::KEY_LEFTMETA),
        ],
    );
    assert_actions(
        APP_SCOPED,
        chord,
        vec![
            press(Key::KEY_LEFTMETA),
            press(Key::KEY_LEFTCTRL),
            release(Key::KEY_LEFTMETA),
            press(Key::KEY_C),
            release(Key::KEY_C),
            delay(),
            press(Key::KEY_LEFTMETA),
            delay(),
            release(Key::KEY_LEFTCTRL),
            release(Key::KEY_C),
            release(Key::KEY_LEFTMETA),
        ],
    );
}

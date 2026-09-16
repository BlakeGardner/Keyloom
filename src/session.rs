//! Which desktop this session runs on, as far as xremap cares.
//!
//! xremap asks the compositor which window is in front through a
//! desktop-specific client, chosen with `--desktop` (or by trying each
//! one in turn when the flag is absent). Keyloom reads the same
//! environment the desktop set up for this session to name that
//! desktop, and to know whether the remapping service should wait for a
//! Wayland socket at all. Every value is a parameter of [`Session::from_env`]
//! so the rules can be tested without touching the environment.

/// A desktop xremap has a client for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Kde,
    Cosmic,
    Hypr,
    Niri,
    Pantheon,
    /// Any compositor speaking the wlr-foreign-toplevel protocol: sway,
    /// river, labwc, and friends.
    Wlroots,
    /// An X11 session, whatever desktop runs on it.
    X11,
}

/// `XDG_CURRENT_DESKTOP` entries and the desktop each names. Matched
/// without regard to case; a value like `ubuntu:GNOME` is tried entry
/// by entry.
const CURRENT_DESKTOP_NAMES: &[(&str, Desktop)] = &[
    ("cosmic", Desktop::Cosmic),
    ("gnome", Desktop::Gnome),
    ("gnome-classic", Desktop::Gnome),
    ("kde", Desktop::Kde),
    ("hyprland", Desktop::Hypr),
    ("niri", Desktop::Niri),
    ("pantheon", Desktop::Pantheon),
    ("sway", Desktop::Wlroots),
    ("river", Desktop::Wlroots),
    ("labwc", Desktop::Wlroots),
    ("wayfire", Desktop::Wlroots),
    ("dwl", Desktop::Wlroots),
];

impl Desktop {
    /// Every desktop, in the order `xremap --list-desktops` prints them.
    pub const ALL: [Self; 8] = [
        Self::Gnome,
        Self::Kde,
        Self::Hypr,
        Self::Niri,
        Self::Wlroots,
        Self::Cosmic,
        Self::Pantheon,
        Self::X11,
    ];

    /// The value xremap's `--desktop` flag takes.
    pub const fn flag(self) -> &'static str {
        match self {
            Self::Gnome => "gnome",
            Self::Kde => "kde",
            Self::Cosmic => "cosmic",
            Self::Hypr => "hypr",
            Self::Niri => "niri",
            Self::Pantheon => "pantheon",
            Self::Wlroots => "wlroots",
            Self::X11 => "x11",
        }
    }

    /// The desktop's name in a sentence.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Gnome => "GNOME",
            Self::Kde => "KDE Plasma",
            Self::Cosmic => "COSMIC",
            Self::Hypr => "Hyprland",
            Self::Niri => "niri",
            Self::Pantheon => "Pantheon",
            Self::Wlroots => "a wlroots compositor",
            Self::X11 => "X11",
        }
    }

    /// The name `xremap --list-desktops` prints for this client.
    const fn list_name(self) -> &'static str {
        match self {
            Self::Gnome => "GNOME",
            Self::Kde => "KDE",
            Self::Cosmic => "COSMIC",
            Self::Hypr => "Hypr",
            Self::Niri => "Niri",
            Self::Pantheon => "Pantheon",
            Self::Wlroots => "wlroots",
            Self::X11 => "X11",
        }
    }

    /// The desktop behind a name from `xremap --list-desktops`, compared
    /// without regard to case. Names that are not desktops (the `Socket`
    /// bridge) give `None`.
    pub fn from_list_name(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|desktop| name.eq_ignore_ascii_case(desktop.list_name()))
    }

    /// The desktop an `XDG_CURRENT_DESKTOP` value names, if xremap has a
    /// client for it: the first recognized entry of the colon-separated
    /// list wins.
    pub fn from_current_desktop(value: &str) -> Option<Self> {
        value
            .split(':')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .find_map(|entry| {
                CURRENT_DESKTOP_NAMES
                    .iter()
                    .find(|(name, _)| entry.eq_ignore_ascii_case(name))
                    .map(|(_, desktop)| *desktop)
            })
    }
}

/// What the session's environment says about the desktop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Session {
    /// The desktop xremap should connect to; `None` leaves the choice
    /// to xremap.
    pub desktop: Option<Desktop>,
    /// The session runs on X11 rather than Wayland, so no Wayland socket
    /// will ever appear for a service to wait on.
    pub x11: bool,
}

impl Session {
    /// Read the desktop's environment: `XDG_CURRENT_DESKTOP`,
    /// `XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, and `DISPLAY`.
    pub fn detect() -> Self {
        let var = |name: &str| std::env::var(name).ok();
        Self::from_env(
            var("XDG_CURRENT_DESKTOP").as_deref(),
            var("XDG_SESSION_TYPE").as_deref(),
            var("WAYLAND_DISPLAY").as_deref(),
            var("DISPLAY").as_deref(),
        )
    }

    /// Decide from the environment values. The session is X11 when
    /// `XDG_SESSION_TYPE` says so, or when it says nothing useful and
    /// only an X display is set. On X11 the desktop is [`Desktop::X11`]
    /// whatever runs on it; on Wayland it comes from
    /// `XDG_CURRENT_DESKTOP`.
    pub fn from_env(
        current_desktop: Option<&str>,
        session_type: Option<&str>,
        wayland_display: Option<&str>,
        display: Option<&str>,
    ) -> Self {
        fn set(value: Option<&str>) -> Option<&str> {
            value.map(str::trim).filter(|value| !value.is_empty())
        }
        let x11 = match set(session_type) {
            Some(kind) if kind.eq_ignore_ascii_case("x11") => true,
            Some(kind) if kind.eq_ignore_ascii_case("wayland") => false,
            _ => set(wayland_display).is_none() && set(display).is_some(),
        };
        let desktop = if x11 {
            Some(Desktop::X11)
        } else {
            set(current_desktop).and_then(Desktop::from_current_desktop)
        };
        Self { desktop, x11 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wayland(current_desktop: &str) -> Session {
        Session::from_env(
            Some(current_desktop),
            Some("wayland"),
            Some("wayland-1"),
            Some(":1"),
        )
    }

    #[test]
    fn wayland_desktops_are_named_from_xdg_current_desktop() {
        assert_eq!(
            wayland("COSMIC"),
            Session {
                desktop: Some(Desktop::Cosmic),
                x11: false,
            }
        );
        assert_eq!(wayland("ubuntu:GNOME").desktop, Some(Desktop::Gnome));
        assert_eq!(wayland("GNOME-Classic:GNOME").desktop, Some(Desktop::Gnome));
        assert_eq!(wayland("X-Vendor:KDE").desktop, Some(Desktop::Kde));
        assert_eq!(wayland("Hyprland").desktop, Some(Desktop::Hypr));
        assert_eq!(wayland("niri").desktop, Some(Desktop::Niri));
        assert_eq!(wayland("Pantheon").desktop, Some(Desktop::Pantheon));
        assert_eq!(wayland("sway").desktop, Some(Desktop::Wlroots));
        assert_eq!(wayland("river").desktop, Some(Desktop::Wlroots));
        assert_eq!(wayland("cosmic").desktop, Some(Desktop::Cosmic), "case");
        assert_eq!(
            wayland(" : COSMIC : ").desktop,
            Some(Desktop::Cosmic),
            "empty entries"
        );
        assert_eq!(wayland("XFCE").desktop, None, "no client for it");
        assert_eq!(wayland("cosmic-ish").desktop, None, "whole names only");
        assert_eq!(wayland("").desktop, None);
    }

    #[test]
    fn x11_sessions_use_the_x11_client_whatever_the_desktop() {
        let x11 = Session::from_env(Some("pop:GNOME"), Some("x11"), None, Some(":0"));
        assert_eq!(
            x11,
            Session {
                desktop: Some(Desktop::X11),
                x11: true,
            }
        );
        assert_eq!(
            Session::from_env(Some("KDE"), Some("X11"), Some("wayland-0"), Some(":0")),
            x11,
            "the session type has the last word, whatever else is set"
        );
    }

    #[test]
    fn the_displays_decide_when_the_session_type_is_missing() {
        // Only an X display: an X11 session started without a session
        // manager, say.
        assert!(Session::from_env(Some("GNOME"), None, None, Some(":0")).x11);
        assert!(Session::from_env(Some("GNOME"), Some(""), Some(" "), Some(":0")).x11);
        // A Wayland socket, with or without XWayland's display.
        let wayland = Session::from_env(Some("GNOME"), None, Some("wayland-0"), Some(":0"));
        assert_eq!(
            wayland,
            Session {
                desktop: Some(Desktop::Gnome),
                x11: false,
            }
        );
        assert!(!Session::from_env(Some("GNOME"), Some("tty"), Some("wayland-0"), None).x11);
        // Nothing at all: not X11, and no desktop to name.
        assert_eq!(
            Session::from_env(None, None, None, None),
            Session::default()
        );
    }

    #[test]
    fn list_names_round_trip_and_the_bridge_is_not_a_desktop() {
        for desktop in Desktop::ALL {
            assert_eq!(Desktop::from_list_name(desktop.list_name()), Some(desktop));
            assert_eq!(
                Desktop::from_list_name(&desktop.list_name().to_ascii_uppercase()),
                Some(desktop)
            );
        }
        assert_eq!(Desktop::from_list_name(" wlroots "), Some(Desktop::Wlroots));
        assert_eq!(Desktop::from_list_name("Socket"), None);
        assert_eq!(Desktop::from_list_name(""), None);
    }

    #[test]
    fn every_desktop_has_its_own_flag_and_label() {
        for (index, desktop) in Desktop::ALL.into_iter().enumerate() {
            assert!(!desktop.label().is_empty());
            assert!(
                desktop
                    .flag()
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()),
                "{desktop:?}: flags are plain lowercase words"
            );
            for other in &Desktop::ALL[index + 1..] {
                assert_ne!(desktop.flag(), other.flag());
                assert_ne!(desktop.label(), other.label());
            }
        }
    }
}

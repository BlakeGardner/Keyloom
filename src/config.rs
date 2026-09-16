//! Keyloom's persistent configuration model, stored via cosmic-config.
//!
//! This internal model — not the generated xremap YAML — is the source
//! of truth for profiles and their mappings. It is saved under
//! `~/.config/cosmic/io.github.blakegardner.Keyloom/` and reloaded on
//! the next launch; the xremap file is regenerated from it.

use std::collections::HashMap;

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

use crate::monitor::KeyboardId;
use crate::ui::model::{Layer, Maps, Profile};

/// The application id, shared with the `cosmic::Application` impl.
pub const APP_ID: &str = "io.github.blakegardner.Keyloom";

/// One stored profile: identity plus its ordered key mappings and
/// layers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredProfile {
    pub id: String,
    pub name: String,
    pub mappings: Maps,
    /// Absent from stores written before layers existed.
    #[serde(default)]
    pub layers: Vec<Layer>,
}

/// The app's in-memory profile state, as loaded from the store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileState {
    pub profiles: Vec<Profile>,
    pub maps: HashMap<String, Maps>,
    pub layers: HashMap<String, Vec<Layer>>,
    /// The active profile id, falling back to the first profile.
    pub active: String,
    pub custom_profiles: u32,
}

/// Manual display choices; each absent value follows automatic detection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutOverride {
    pub form: Option<usize>,
    pub iso: Option<bool>,
}

/// Display preferences belong to keyboards, independently of remap profiles.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardLayouts {
    pub all: LayoutOverride,
    pub devices: HashMap<KeyboardId, LayoutOverride>,
}

/// Where first-run setup stands, so it opens on its own only until the
/// user has been through it once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetupState {
    /// Never shown: it opens with the next launch.
    #[default]
    NotStarted,
    /// Closed before everything was in order; reopen it from the menu.
    Deferred,
    /// Every step was in order (or only waiting for a new login).
    Complete,
}

/// Everything Keyloom persists between sessions.
#[derive(Clone, Debug, Default, PartialEq, Eq, CosmicConfigEntry)]
#[version = 1]
pub struct KeyloomConfig {
    /// Profile that was active when the app last saved.
    pub active_profile: String,
    /// All profiles with their mappings; empty on a fresh install.
    pub profiles: Vec<StoredProfile>,
    /// Counter used to mint unique ids for user-created profiles.
    pub custom_profiles: u32,
    pub keyboard_layouts: KeyboardLayouts,
    pub setup: SetupState,
}

impl KeyloomConfig {
    /// Open the cosmic-config store for this app and version.
    pub fn handle() -> Option<cosmic_config::Config> {
        match cosmic_config::Config::new(APP_ID, Self::VERSION) {
            Ok(handle) => Some(handle),
            Err(err) => {
                eprintln!("keyloom: settings store unavailable: {err}");
                None
            }
        }
    }

    /// Load the stored entry, falling back to defaults for anything
    /// missing (a fresh install has no stored fields at all).
    pub fn load(handle: &cosmic_config::Config) -> Self {
        match Self::get_entry(handle) {
            Ok(config) => config,
            Err((_, config)) => config,
        }
    }

    /// Snapshot the app's in-memory profile state for saving.
    pub fn snapshot(
        state: &ProfileState,
        keyboard_layouts: &KeyboardLayouts,
        setup: SetupState,
    ) -> Self {
        Self {
            active_profile: state.active.clone(),
            profiles: state
                .profiles
                .iter()
                .map(|profile| StoredProfile {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                    mappings: state.maps.get(&profile.id).cloned().unwrap_or_default(),
                    layers: state.layers.get(&profile.id).cloned().unwrap_or_default(),
                })
                .collect(),
            custom_profiles: state.custom_profiles,
            keyboard_layouts: keyboard_layouts.clone(),
            setup,
        }
    }

    /// Expand the stored entry back into the app's in-memory state.
    pub fn into_state(self) -> ProfileState {
        let mut state = ProfileState {
            custom_profiles: self.custom_profiles,
            ..ProfileState::default()
        };
        for stored in self.profiles {
            state.profiles.push(Profile {
                id: stored.id.clone(),
                name: stored.name,
            });
            state.maps.insert(stored.id.clone(), stored.mappings);
            state.layers.insert(stored.id, stored.layers);
        }
        state.active = if state
            .profiles
            .iter()
            .any(|profile| profile.id == self.active_profile)
        {
            self.active_profile
        } else {
            state
                .profiles
                .first()
                .map_or_else(|| "default".to_owned(), |profile| profile.id.clone())
        };
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::model::{Mapping, navigation_layer};

    #[test]
    fn snapshot_round_trips_through_state() {
        let profiles = vec![
            Profile {
                id: "default".to_owned(),
                name: "Default".to_owned(),
            },
            Profile {
                id: "custom-1".to_owned(),
                name: "Untitled 1".to_owned(),
            },
        ];
        let mut maps = HashMap::new();
        maps.insert(
            "custom-1".to_owned(),
            vec![(
                "CapsLock".to_owned(),
                Mapping {
                    tap: Some("Escape".to_owned()),
                    device: "all".to_owned(),
                    ..Mapping::default()
                },
            )],
        );
        let layers = HashMap::from([("custom-1".to_owned(), vec![navigation_layer()])]);
        let state = ProfileState {
            profiles: profiles.clone(),
            maps: maps.clone(),
            layers: layers.clone(),
            active: "custom-1".to_owned(),
            custom_profiles: 1,
        };

        let layouts = KeyboardLayouts {
            all: LayoutOverride {
                form: Some(3),
                iso: Some(true),
            },
            ..KeyboardLayouts::default()
        };
        let snapshot = KeyloomConfig::snapshot(&state, &layouts, SetupState::Deferred);
        assert_eq!(
            snapshot.keyboard_layouts, layouts,
            "profile saves preserve display preferences"
        );
        assert_eq!(snapshot.setup, SetupState::Deferred);
        let restored = snapshot.into_state();

        assert_eq!(restored.profiles, profiles);
        assert_eq!(restored.maps.get("custom-1"), maps.get("custom-1"));
        assert_eq!(restored.maps.get("default"), Some(&Vec::new()));
        assert_eq!(restored.layers.get("custom-1"), layers.get("custom-1"));
        assert_eq!(
            restored.layers.get("default"),
            Some(&Vec::new()),
            "every profile gets a layer list"
        );
        assert_eq!(restored.active, "custom-1");
        assert_eq!(restored.custom_profiles, 1);
    }

    #[test]
    fn unknown_active_profile_falls_back_to_the_first() {
        let config = KeyloomConfig {
            active_profile: "gone".to_owned(),
            profiles: vec![StoredProfile {
                id: "default".to_owned(),
                name: "Default".to_owned(),
                mappings: Vec::new(),
                layers: Vec::new(),
            }],
            custom_profiles: 0,
            ..KeyloomConfig::default()
        };
        assert_eq!(config.into_state().active, "default");
    }

    #[test]
    fn write_and_load_round_trip_through_the_store() {
        let dir = std::env::temp_dir().join(format!("keyloom-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let handle =
            cosmic_config::Config::with_custom_path(APP_ID, KeyloomConfig::VERSION, dir.clone())
                .expect("custom-path store");

        let config = KeyloomConfig {
            active_profile: "laptop".to_owned(),
            profiles: vec![StoredProfile {
                id: "laptop".to_owned(),
                name: "Laptop".to_owned(),
                mappings: vec![(
                    "CapsLock".to_owned(),
                    Mapping {
                        tap: Some("Escape".to_owned()),
                        hold: Some("Control".to_owned()),
                        device: "all".to_owned(),
                        swap: false,
                    },
                )],
                layers: vec![navigation_layer()],
            }],
            custom_profiles: 3,
            keyboard_layouts: KeyboardLayouts {
                all: LayoutOverride {
                    form: Some(1),
                    iso: None,
                },
                devices: HashMap::from([(
                    KeyboardId::new(
                        evdev::InputId::new(evdev::BusType::BUS_USB, 1, 2, 1),
                        Some("serial-1"),
                        None,
                        "Keyboard",
                    ),
                    LayoutOverride {
                        form: Some(3),
                        iso: Some(true),
                    },
                )]),
            },
            setup: SetupState::Complete,
        };
        config.write_entry(&handle).unwrap();
        assert_eq!(KeyloomConfig::load(&handle), config);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn existing_store_without_keyboard_layouts_preserves_profiles() {
        use cosmic_config::ConfigSet;
        let dir =
            std::env::temp_dir().join(format!("keyloom-legacy-config-test-{}", std::process::id()));
        let handle =
            cosmic_config::Config::with_custom_path(APP_ID, KeyloomConfig::VERSION, dir.clone())
                .unwrap();
        let profiles = vec![StoredProfile {
            id: "existing".to_owned(),
            name: "Existing profile".to_owned(),
            mappings: Vec::new(),
            layers: Vec::new(),
        }];
        handle.set("profiles", &profiles).unwrap();
        handle.set("active_profile", "existing").unwrap();
        handle.set("custom_profiles", 5_u32).unwrap();

        let loaded = KeyloomConfig::load(&handle);
        assert_eq!(loaded.profiles, profiles);
        assert_eq!(loaded.active_profile, "existing");
        assert_eq!(loaded.custom_profiles, 5);
        assert_eq!(loaded.keyboard_layouts, KeyboardLayouts::default());
        assert_eq!(
            loaded.setup,
            SetupState::NotStarted,
            "an existing store without a setup record has not been through setup"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Profiles saved before layers existed have no `layers` field;
    /// they load with none rather than failing.
    #[test]
    fn profiles_stored_without_layers_still_load() {
        use cosmic_config::ConfigSet;
        let dir = std::env::temp_dir().join(format!(
            "keyloom-prelayer-config-test-{}",
            std::process::id()
        ));
        let handle =
            cosmic_config::Config::with_custom_path(APP_ID, KeyloomConfig::VERSION, dir.clone())
                .unwrap();
        #[derive(Serialize)]
        struct OldProfile {
            id: String,
            name: String,
            mappings: Maps,
        }
        let old = vec![OldProfile {
            id: "laptop".to_owned(),
            name: "Laptop".to_owned(),
            mappings: vec![(
                "CapsLock".to_owned(),
                Mapping {
                    tap: Some("Escape".to_owned()),
                    device: "all".to_owned(),
                    ..Mapping::default()
                },
            )],
        }];
        handle.set("profiles", &old).unwrap();

        let loaded = KeyloomConfig::load(&handle);
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.profiles[0].name, "Laptop");
        assert_eq!(loaded.profiles[0].mappings.len(), 1);
        assert!(loaded.profiles[0].layers.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

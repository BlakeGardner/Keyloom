//! Keyloom's persistent configuration model, stored via cosmic-config.
//!
//! This internal model — not the generated xremap YAML — is the source
//! of truth for profiles and their mappings. It is saved under
//! `~/.config/cosmic/io.github.blakegardner.Keyloom/` and reloaded on
//! the next launch; the xremap file is regenerated from it.

use std::collections::HashMap;

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

use crate::ui::model::{Maps, Profile};

/// The application id, shared with the `cosmic::Application` impl.
pub const APP_ID: &str = "io.github.blakegardner.Keyloom";

/// One stored profile: identity plus its ordered key mappings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredProfile {
    pub id: String,
    pub name: String,
    pub mappings: Maps,
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
        profiles: &[Profile],
        profile_maps: &HashMap<String, Maps>,
        active_profile: &str,
        custom_profiles: u32,
    ) -> Self {
        Self {
            active_profile: active_profile.to_owned(),
            profiles: profiles
                .iter()
                .map(|profile| StoredProfile {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                    mappings: profile_maps.get(&profile.id).cloned().unwrap_or_default(),
                })
                .collect(),
            custom_profiles,
        }
    }

    /// Expand the stored entry back into the app's in-memory state.
    pub fn into_state(self) -> (Vec<Profile>, HashMap<String, Maps>, String, u32) {
        let mut profiles = Vec::new();
        let mut profile_maps = HashMap::new();
        for stored in self.profiles {
            profiles.push(Profile {
                id: stored.id.clone(),
                name: stored.name,
            });
            profile_maps.insert(stored.id, stored.mappings);
        }
        let active = if profiles.iter().any(|profile| profile.id == self.active_profile) {
            self.active_profile
        } else {
            profiles
                .first()
                .map_or_else(|| "default".to_owned(), |profile| profile.id.clone())
        };
        (profiles, profile_maps, active, self.custom_profiles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::model::Mapping;

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

        let snapshot = KeyloomConfig::snapshot(&profiles, &maps, "custom-1", 1);
        let (restored, restored_maps, active, custom) = snapshot.into_state();

        assert_eq!(restored, profiles);
        assert_eq!(restored_maps.get("custom-1"), maps.get("custom-1"));
        assert_eq!(restored_maps.get("default"), Some(&Vec::new()));
        assert_eq!(active, "custom-1");
        assert_eq!(custom, 1);
    }

    #[test]
    fn unknown_active_profile_falls_back_to_the_first() {
        let config = KeyloomConfig {
            active_profile: "gone".to_owned(),
            profiles: vec![StoredProfile {
                id: "default".to_owned(),
                name: "Default".to_owned(),
                mappings: Vec::new(),
            }],
            custom_profiles: 0,
        };
        let (_, _, active, _) = config.into_state();
        assert_eq!(active, "default");
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
            }],
            custom_profiles: 3,
        };
        config.write_entry(&handle).unwrap();
        assert_eq!(KeyloomConfig::load(&handle), config);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

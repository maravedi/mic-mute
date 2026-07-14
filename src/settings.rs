use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    #[serde(default)]
    pub modifiers: Vec<String>, // ["shift", "meta", "ctrl", "alt"]
    pub key: String, // "A", "M", "F13", etc.
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            modifiers: vec!["shift".to_string(), "meta".to_string()],
            key: "A".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub mic_shortcut: ShortcutConfig,
    #[serde(default)]
    pub show_in_dock: bool,
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default = "default_show_popup")]
    pub show_popup: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mic_shortcut: ShortcutConfig::default(),
            show_in_dock: false,
            launch_at_login: false,
            show_popup: true,
        }
    }
}

fn default_show_popup() -> bool {
    true
}

/// Returns migrated JSON when the popup setting is missing.
fn seed_show_popup_setting(data: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(data).ok()?;
    let object = value.as_object_mut()?;
    if object.contains_key("show_popup") {
        return None;
    }

    object.insert("show_popup".to_string(), serde_json::Value::Bool(true));
    serde_json::to_string_pretty(&value).ok()
}

impl Settings {
    pub fn load() -> Self {
        Self::load_from_file().unwrap_or_default()
    }

    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("mic-mute").join("settings.json"))
    }

    fn load_from_file() -> Option<Self> {
        let path = Self::config_path()?;
        Self::load_from_path(&path)
    }

    fn load_from_path(path: &Path) -> Option<Self> {
        let data = std::fs::read_to_string(path).ok()?;
        Self::load_from_data(&data, |migrated_data| std::fs::write(path, migrated_data))
    }

    fn load_from_data<F>(data: &str, write_migration: F) -> Option<Self>
    where
        F: FnOnce(String) -> std::io::Result<()>,
    {
        let settings = serde_json::from_str(data).ok()?;

        if let Some(migrated_data) = seed_show_popup_setting(data) {
            if let Err(error) = write_migration(migrated_data) {
                log::warn!("Failed to seed show_popup setting: {error}");
            }
        }

        Some(settings)
    }

    /// Returns the last-modified time of the settings file, or None if it doesn't exist.
    pub fn mtime() -> Option<std::time::SystemTime> {
        Self::config_path()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
    }

    pub fn save(&self) -> Result<()> {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let data = serde_json::to_string_pretty(self)?;
            std::fs::write(path, data)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_shortcut() {
        let sc = ShortcutConfig::default();
        assert_eq!(sc.key, "A");
        assert!(sc.modifiers.contains(&"shift".to_string()));
        assert!(sc.modifiers.contains(&"meta".to_string()));
    }
    #[test]
    fn test_default_settings_show_popup() {
        assert!(Settings::default().show_popup);
    }

    #[test]
    fn test_settings_json_missing_show_popup_defaults_to_enabled() {
        let loaded: Settings = serde_json::from_str(
            r#"{
                "mic_shortcut": {
                    "key": "F13"
                }
            }"#,
        )
        .unwrap();

        assert!(loaded.show_popup);
    }

    #[test]
    fn test_seed_show_popup_setting_preserves_unknown_fields() {
        let migrated = seed_show_popup_setting(
            r#"{
                "future_setting": "preserve me"
            }"#,
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&migrated).unwrap();

        assert_eq!(value["show_popup"], serde_json::Value::Bool(true));
        assert_eq!(
            value["future_setting"],
            serde_json::Value::String("preserve me".to_string())
        );
    }

    #[test]
    fn test_seed_show_popup_setting_skips_existing_values() {
        assert!(seed_show_popup_setting(r#"{"show_popup": false}"#).is_none());
        assert!(seed_show_popup_setting(r#"{"show_popup": true}"#).is_none());
    }

    #[test]
    fn test_seed_show_popup_setting_skips_invalid_json() {
        assert!(seed_show_popup_setting("not json").is_none());
        assert!(seed_show_popup_setting("[]").is_none());
    }

    #[test]
    fn test_load_from_path_seeds_missing_show_popup() {
        use std::fs;

        let path = std::env::temp_dir().join(format!(
            "mic-mute-test-settings-migration-{}.json",
            std::process::id()
        ));
        let original = r#"{
            "future_setting": "preserve me"
        }"#;
        fs::write(&path, original).unwrap();

        let loaded = Settings::load_from_path(&path).unwrap();
        let migrated: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

        assert!(loaded.show_popup);
        assert_eq!(migrated["show_popup"], serde_json::Value::Bool(true));
        assert_eq!(
            migrated["future_setting"],
            serde_json::Value::String("preserve me".to_string())
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_load_from_data_keeps_settings_when_migration_write_fails() {
        let loaded = Settings::load_from_data(r#"{"show_in_dock": true}"#, |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "read-only",
            ))
        })
        .unwrap();

        assert!(loaded.show_in_dock);
        assert!(loaded.show_popup);
    }

    #[test]
    fn test_load_from_path_does_not_rewrite_existing_show_popup() {
        use std::fs;

        for (suffix, show_popup) in [("false", false), ("true", true)] {
            let path = std::env::temp_dir().join(format!(
                "mic-mute-test-settings-existing-{}-{}.json",
                std::process::id(),
                suffix
            ));
            let original =
                format!(r#"{{"show_popup": {show_popup}, "future_setting": "preserve"}}"#);
            fs::write(&path, &original).unwrap();

            let loaded = Settings::load_from_path(&path).unwrap();
            let after = fs::read_to_string(&path).unwrap();

            assert_eq!(loaded.show_popup, show_popup);
            assert_eq!(after, original);
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn test_load_from_path_skips_invalid_json() {
        use std::fs;

        let path = std::env::temp_dir().join(format!(
            "mic-mute-test-settings-invalid-{}.json",
            std::process::id()
        ));
        let original = "not json";
        fs::write(&path, original).unwrap();

        assert!(Settings::load_from_path(&path).is_none());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_load_from_path_does_not_migrate_non_object_json() {
        use std::fs;

        let path = std::env::temp_dir().join(format!(
            "mic-mute-test-settings-array-{}.json",
            std::process::id()
        ));
        let original = "[]";
        fs::write(&path, original).unwrap();

        let _ = Settings::load_from_path(&path);
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_settings_json_round_trip() {
        let s = Settings::default();

        let json = serde_json::to_string(&s).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.mic_shortcut.key, "A");
        assert!(loaded.show_popup);
    }

    #[test]
    fn test_settings_json_missing_shortcut_modifiers() {
        let loaded: Settings = serde_json::from_str(
            r#"{
                "mic_shortcut": {
                    "key": "F13"
                }
            }"#,
        )
        .unwrap();

        assert_eq!(loaded.mic_shortcut.key, "F13");
        assert!(loaded.mic_shortcut.modifiers.is_empty());
    }

    #[test]
    fn test_settings_save_and_load() {
        use std::fs;

        // Use a temp path for testing
        let tmp_dir = std::env::temp_dir().join("mic-mute-test-settings");
        let tmp_path = tmp_dir.join("settings.json");
        let _ = fs::remove_file(&tmp_path);
        let _ = fs::create_dir_all(&tmp_dir);

        let s = Settings {
            mic_shortcut: ShortcutConfig {
                modifiers: vec!["shift".to_string()],
                key: "M".to_string(),
            },
            show_in_dock: false,
            launch_at_login: false,
            show_popup: false,
        };

        let json = serde_json::to_string_pretty(&s).unwrap();
        fs::write(&tmp_path, &json).unwrap();

        let loaded: Settings =
            serde_json::from_str(&fs::read_to_string(&tmp_path).unwrap()).unwrap();
        assert_eq!(loaded.mic_shortcut.key, "M");
        assert!(!loaded.show_popup);

        let _ = fs::remove_file(&tmp_path);
    }
}

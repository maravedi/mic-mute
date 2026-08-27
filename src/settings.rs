use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// A screen-relative anchor for the mute-status overlay.
///
/// The overlay always follows the display containing the cursor; this setting
/// only controls where it is placed within that display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayPosition {
    TopLeft,
    #[default]
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

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
    #[serde(default)]
    pub diagnostic_logging: bool,
    #[serde(default)]
    pub overlay_position: OverlayPosition,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mic_shortcut: ShortcutConfig::default(),
            show_in_dock: false,
            launch_at_login: false,
            show_popup: true,
            diagnostic_logging: false,
            overlay_position: OverlayPosition::default(),
        }
    }
}

fn default_show_popup() -> bool {
    true
}

impl Settings {
    pub fn load() -> Self {
        Self::load_from_file().unwrap_or_default()
    }

    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("mic-mute").join("settings.json"))
    }

    fn load_from_file() -> Option<Self> {
        let path = Self::path()?;
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Returns the last-modified time of the settings file, or None if it doesn't exist.
    pub fn mtime() -> Option<std::time::SystemTime> {
        Self::path()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
    }

    pub fn save(&self) -> Result<()> {
        if let Some(path) = Self::path() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let data = serde_json::to_string_pretty(self)?;
            std::fs::write(path, data)?;
        }
        Ok(())
    }

    /// Opens the settings JSON in the user's default text editor. Persist a
    /// default configuration first so this also works before the first change.
    pub fn open_in_editor(&self) -> Result<()> {
        let path = Self::path().ok_or_else(|| anyhow::anyhow!("Settings directory unavailable"))?;
        if !path.exists() {
            self.save()?;
        }
        Command::new("open").arg("-t").arg(path).spawn()?;
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
        assert!(!Settings::default().diagnostic_logging);
        assert_eq!(
            Settings::default().overlay_position,
            OverlayPosition::TopCenter
        );
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
        assert_eq!(loaded.overlay_position, OverlayPosition::TopCenter);
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
            diagnostic_logging: true,
            overlay_position: OverlayPosition::BottomRight,
        };

        let json = serde_json::to_string_pretty(&s).unwrap();
        fs::write(&tmp_path, &json).unwrap();

        let loaded: Settings =
            serde_json::from_str(&fs::read_to_string(&tmp_path).unwrap()).unwrap();
        assert_eq!(loaded.mic_shortcut.key, "M");
        assert!(!loaded.show_popup);
        assert!(loaded.diagnostic_logging);
        assert_eq!(loaded.overlay_position, OverlayPosition::BottomRight);

        let _ = fs::remove_file(&tmp_path);
    }
}

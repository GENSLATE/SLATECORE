//! Writing one setting from the UI into `settings.toml`, keeping the file's comments. The key
//! and the value are checked against a fixed list first; the file watcher (and a direct reload)
//! then applies the change like any hand edit. No Tauri types, so it is tested on any host.

use std::path::Path;

use genslate_launcher_core::LauncherError;
use genslate_launcher_core::metadata::write_value;
use serde::Deserialize;

/// A setting the UI may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingKey {
    Theme,
    Size,
    StatusMode,
}

/// Where a key lives in `settings.toml` and which values it accepts.
struct Slot {
    table: &'static str,
    name: &'static str,
    allowed: &'static [&'static str],
}

const fn slot(key: SettingKey) -> Slot {
    match key {
        SettingKey::Theme => Slot {
            table: "appearance",
            name: "theme",
            allowed: &["system", "polar-night", "snow-storm"],
        },
        SettingKey::Size => Slot {
            table: "appearance",
            name: "size",
            allowed: &["s", "m", "l"],
        },
        SettingKey::StatusMode => Slot {
            table: "status",
            name: "mode",
            allowed: &["temps", "usage"],
        },
    }
}

/// The theme word of `/theme <mode>` (`dark`, `light`, `system`) as `settings.toml` spells it.
pub fn theme_for_mode(mode: &str) -> &'static str {
    match mode {
        "dark" => "polar-night",
        "light" => "snow-storm",
        _ => "system",
    }
}

/// Checks `value` and writes `key` into the settings file at `file` (created when missing).
///
/// # Errors
///
/// `InvalidParams` for a value the key does not accept, else the write error.
pub fn write_setting(file: &Path, key: SettingKey, value: &str) -> Result<(), LauncherError> {
    let slot = slot(key);
    if !slot.allowed.contains(&value) {
        return Err(LauncherError::InvalidParams {
            action: "set_setting".to_owned(),
            message: format!("{} must be one of {}", slot.name, slot.allowed.join(", ")),
        });
    }
    write_value(file, slot.table, slot.name, value.into())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_launcher_core::config::LauncherConfig;
    use genslate_testing::TempTree;
    use std::error::Error;

    #[test]
    fn a_valid_theme_is_written_with_the_files_comments_kept() -> Result<(), Box<dyn Error>> {
        let tmp = TempTree::new()?.file(
            "settings.toml",
            "# my notes\n[appearance]\ntheme = \"system\" # why\n",
        )?;
        let file = tmp.join("settings.toml");

        write_setting(&file, SettingKey::Theme, "polar-night")?;

        let text = std::fs::read_to_string(&file)?;
        assert!(text.contains("# my notes"), "{text}");
        assert!(text.contains("# why"), "{text}");
        let config = LauncherConfig::load(&file)?;
        assert_eq!(
            serde_json::to_value(&config.appearance)?["theme"],
            "polar-night"
        );
        Ok(())
    }

    #[test]
    fn the_file_is_created_when_it_does_not_exist() -> Result<(), Box<dyn Error>> {
        let tmp = TempTree::new()?;
        let file = tmp.join("configs").join("settings.toml");
        write_setting(&file, SettingKey::Size, "l")?;
        write_setting(&file, SettingKey::StatusMode, "temps")?;
        let config = LauncherConfig::load(&file)?;
        assert_eq!(serde_json::to_value(&config.appearance)?["size"], "l");
        assert_eq!(serde_json::to_value(&config.status)?["mode"], "temps");
        Ok(())
    }

    #[test]
    fn a_value_outside_the_list_is_refused_and_nothing_is_written() -> Result<(), Box<dyn Error>> {
        let tmp = TempTree::new()?;
        let file = tmp.join("settings.toml");
        for (key, value) in [
            (SettingKey::Theme, "purple"),
            (SettingKey::Size, "xl"),
            (SettingKey::StatusMode, "true"),
            (SettingKey::Theme, ""),
        ] {
            let error = write_setting(&file, key, value).err().ok_or("accepted")?;
            assert_eq!(error.kind(), "invalid-params");
        }
        assert!(!file.exists());
        Ok(())
    }

    #[test]
    fn slash_theme_words_map_to_the_files_spelling() {
        assert_eq!(theme_for_mode("dark"), "polar-night");
        assert_eq!(theme_for_mode("light"), "snow-storm");
        assert_eq!(theme_for_mode("system"), "system");
        assert_eq!(theme_for_mode("anything else"), "system");
    }

    #[test]
    fn the_frontend_setting_keys_deserialize() -> Result<(), serde_json::Error> {
        assert_eq!(
            serde_json::from_str::<SettingKey>("\"statusMode\"")?,
            SettingKey::StatusMode
        );
        assert!(serde_json::from_str::<SettingKey>("\"hideOnBlur\"").is_err());
        Ok(())
    }
}

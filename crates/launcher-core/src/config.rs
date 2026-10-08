//! The launcher's hand-editable settings: `settings.toml` and `keybindings.toml` in
//! `other/launcher/configs/`.
//!
//! Every key is optional (missing = default) and unknown keys are rejected so typos surface;
//! that includes the old `autostart` key, which does not exist here (the launcher never writes
//! to the PC's registry). TOML uses kebab-case keys; the UI receives camelCase JSON.
//!
//! Nothing in these files is a path: drive letters change between PCs.
//!
//! [`Settings`] is the loaded pair of files. When a file is edited into something invalid, the
//! values that were in use stay in use and [`Settings::issue`] says what is wrong.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::LauncherError;
use crate::error::read_error;

/// File name of [`LauncherConfig`] inside `other/launcher/configs/`.
pub const SETTINGS_FILE: &str = "settings.toml";
/// File name of [`Keybindings`] inside `other/launcher/configs/`.
pub const KEYBINDINGS_FILE: &str = "keybindings.toml";

/// `settings.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    rename_all(serialize = "camelCase", deserialize = "kebab-case")
)]
pub struct LauncherConfig {
    /// `[appearance]`
    pub appearance: Appearance,
    /// `[behavior]`
    pub behavior: Behavior,
    /// `[status]`
    pub status: StatusConfig,
    /// `[vault]`
    pub vault: VaultSettings,
    /// `[logging]`
    pub logging: Logging,
}

/// `[appearance]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    rename_all(serialize = "camelCase", deserialize = "kebab-case")
)]
pub struct Appearance {
    /// Colour theme.
    pub theme: ThemePreference,
    /// Window height preset.
    pub size: SizePreset,
}

/// `[behavior]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    rename_all(serialize = "camelCase", deserialize = "kebab-case")
)]
pub struct Behavior {
    /// Hide when another window gets focus (ignored while pinned).
    pub hide_on_blur: bool,
    /// Hide after launching an app (ignored while pinned).
    pub hide_on_launch: bool,
    /// Start pinned: always on top, never auto-hides.
    pub pinned: bool,
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            hide_on_blur: true,
            hide_on_launch: true,
            pinned: false,
        }
    }
}

/// `[status]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    rename_all(serialize = "camelCase", deserialize = "kebab-case")
)]
pub struct StatusConfig {
    /// What the right side of the status bar shows first.
    pub mode: StatusMode,
}

/// `[vault]`: when the launcher locks the vault by itself. The launcher holds the timers and
/// calls the vault's `lock`; the vault crate knows nothing about them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    rename_all(serialize = "camelCase", deserialize = "kebab-case")
)]
pub struct VaultSettings {
    /// Lock after this many minutes without use. `0` = never.
    pub idle_lock_minutes: u32,
    /// Lock after the window has been hidden this many minutes. `0` = never.
    pub hide_lock_minutes: u32,
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            idle_lock_minutes: 10,
            hide_lock_minutes: 5,
        }
    }
}

/// `[logging]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    rename_all(serialize = "camelCase", deserialize = "kebab-case")
)]
pub struct Logging {
    /// Log verbosity.
    pub level: LogLevel,
}

/// `"system" | "polar-night" | "snow-storm"`
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePreference {
    /// Follow the operating system.
    #[default]
    System,
    /// Nord Polar Night (dark).
    PolarNight,
    /// Nord Snow Storm (light).
    SnowStorm,
}

/// Window height: `"s"`, `"m"` or `"l"`; see [`SizePreset::height`] for the pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SizePreset {
    /// Small.
    S,
    /// Medium.
    #[default]
    M,
    /// Large.
    L,
}

/// `"temps" | "usage"`
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusMode {
    /// Temperatures first.
    #[default]
    Temps,
    /// Usage first.
    Usage,
}

/// `"off" | "error" | "warn" | "info" | "debug" | "trace"`
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// No log file.
    Off,
    /// Errors only.
    Error,
    /// Errors and warnings.
    Warn,
    /// The default.
    #[default]
    Info,
    /// Debug detail.
    Debug,
    /// Everything.
    Trace,
}

/// `keybindings.toml`. Shortcut strings use `Ctrl`/`Alt`/`Shift`/`Super` for the global
/// hotkey (OS-level) and the design system's `mod+k` syntax for in-launcher keys.
/// An empty string disables a binding.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    rename_all(serialize = "camelCase", deserialize = "kebab-case")
)]
pub struct Keybindings {
    /// `[global]`
    pub global: GlobalKeys,
    /// `[launcher]`
    pub launcher: LauncherKeys,
}

/// `[global]`: work anywhere in the OS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    rename_all(serialize = "camelCase", deserialize = "kebab-case")
)]
pub struct GlobalKeys {
    /// Show / hide the launcher.
    pub toggle: String,
}

impl Default for GlobalKeys {
    fn default() -> Self {
        Self {
            toggle: "Ctrl+Alt+Space".to_owned(),
        }
    }
}

/// `[launcher]`: while the launcher has focus. The tab keys follow the tab order: GENSLATE,
/// portapps.io, PortableApps.com.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    rename_all(serialize = "camelCase", deserialize = "kebab-case")
)]
pub struct LauncherKeys {
    /// Focus the search bar.
    pub focus_search: String,
    /// Open or close the Settings tool.
    pub toggle_tools: String,
    /// Pin or unpin the window.
    pub toggle_pin: String,
    /// Add the selected app to Favorites or remove it.
    pub toggle_favorite: String,
    /// Show the GENSLATE tab.
    pub tab_genslate: String,
    /// Show the portapps.io tab.
    pub tab_portapps: String,
    /// Show the PortableApps.com tab.
    pub tab_portableapps: String,
}

impl Default for LauncherKeys {
    fn default() -> Self {
        Self {
            focus_search: "mod+k".to_owned(),
            toggle_tools: "mod+,".to_owned(),
            toggle_pin: "mod+p".to_owned(),
            toggle_favorite: "mod+d".to_owned(),
            tab_genslate: "mod+1".to_owned(),
            tab_portapps: "mod+2".to_owned(),
            tab_portableapps: "mod+3".to_owned(),
        }
    }
}

impl LauncherConfig {
    /// Reads `path`; a missing or empty file is the default config.
    pub fn load(path: &Path) -> Result<Self, LauncherError> {
        load_toml(path)
    }

    /// The config as `settings.toml` text (kebab-case keys, readable by [`LauncherConfig::load`]).
    pub fn to_toml(&self) -> Result<String, LauncherError> {
        to_kebab_toml(self)
    }
}

impl Keybindings {
    /// Reads `path`; a missing or empty file is the default keybindings.
    pub fn load(path: &Path) -> Result<Self, LauncherError> {
        load_toml(path)
    }

    /// The keybindings as `keybindings.toml` text.
    pub fn to_toml(&self) -> Result<String, LauncherError> {
        to_kebab_toml(self)
    }
}

/// What the UI receives: both files as loaded, plus why a file was ignored.
///
/// ```
/// use genslate_launcher_core::config::Settings;
///
/// // No files yet: defaults, no issue.
/// let settings = Settings::load(std::path::Path::new("no-such-folder"));
/// assert_eq!(settings.issue, None);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// `settings.toml`.
    pub config: LauncherConfig,
    /// `keybindings.toml`.
    pub keybindings: Keybindings,
    /// Why a settings file was ignored; the previous values stay in use. `None` when both
    /// files are fine. The text names files, never folders.
    pub issue: Option<String>,
}

impl Settings {
    /// Reads both files from `config_dir`. A file that is missing is the default; one that is
    /// invalid is the default too, with [`Settings::issue`] set.
    pub fn load(config_dir: &Path) -> Self {
        let mut settings = Self::default();
        settings.reload(config_dir);
        settings
    }

    /// Reads both files again. Each valid file replaces its values; an invalid one keeps the
    /// values already in use and is reported in [`Settings::issue`], which is cleared as soon
    /// as both files are valid. Returns whether anything the UI shows changed.
    pub fn reload(&mut self, config_dir: &Path) -> bool {
        let before = self.clone();
        let mut issues = Vec::new();
        match LauncherConfig::load(&config_dir.join(SETTINGS_FILE)) {
            Ok(config) => self.config = config,
            Err(error) => issues.push(issue_text(&error)),
        }
        match Keybindings::load(&config_dir.join(KEYBINDINGS_FILE)) {
            Ok(keybindings) => self.keybindings = keybindings,
            Err(error) => issues.push(issue_text(&error)),
        }
        for issue in &issues {
            log::warn!("{issue}; keeping the previous settings");
        }
        self.issue = (!issues.is_empty()).then(|| issues.join("; "));
        *self != before
    }
}

/// One line for the UI: the file's name (not its folder) and what is wrong with it.
fn issue_text(error: &LauncherError) -> String {
    let name = |path: &Path| {
        path.file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned()
    };
    match error {
        LauncherError::Parse { path, message } => format!("{}: {message}", name(path)),
        LauncherError::Read { path, source } => format!("could not read {}: {source}", name(path)),
        other => other.to_string(),
    }
}

/// Parses a TOML settings file into `T`, defaulting when the file is missing.
pub(crate) fn load_toml<T: DeserializeOwned + Default>(path: &Path) -> Result<T, LauncherError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(T::default()),
        Err(error) => return Err(read_error(path)(error)),
    };
    toml::from_str(&text).map_err(|error| LauncherError::Parse {
        path: path.to_path_buf(),
        message: describe(&text, &error),
    })
}

/// `line 3: expected a value`, or just the message when the parser has no position.
fn describe(text: &str, error: &toml::de::Error) -> String {
    match error.span() {
        Some(span) => {
            let before = text.get(..span.start).unwrap_or(text);
            let line = before.matches('\n').count() + 1;
            format!("line {line}: {}", error.message())
        }
        None => error.message().to_owned(),
    }
}

/// Serialises `value` (camelCase for the UI) as TOML with kebab-case keys.
fn to_kebab_toml<T: Serialize>(value: &T) -> Result<String, LauncherError> {
    let value = toml::Value::try_from(value)
        .map_err(|error| LauncherError::Serialize(error.to_string()))?;
    toml::to_string(&kebab_keys(value)).map_err(|error| LauncherError::Serialize(error.to_string()))
}

fn kebab_keys(value: toml::Value) -> toml::Value {
    match value {
        toml::Value::Table(table) => toml::Value::Table(
            table
                .into_iter()
                .map(|(key, value)| (kebab_case(&key), kebab_keys(value)))
                .collect(),
        ),
        toml::Value::Array(items) => {
            toml::Value::Array(items.into_iter().map(kebab_keys).collect())
        }
        other => other,
    }
}

/// `hideOnBlur` → `hide-on-blur`.
fn kebab_case(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    for character in camel.chars() {
        if character.is_ascii_uppercase() {
            out.push('-');
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_testing::TempTree;

    #[test]
    fn missing_and_empty_files_are_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file("empty.toml", "")?;
        assert_eq!(
            LauncherConfig::load(&tree.join("missing.toml"))?,
            LauncherConfig::default()
        );
        assert_eq!(
            LauncherConfig::load(&tree.join("empty.toml"))?,
            LauncherConfig::default()
        );
        assert_eq!(
            Keybindings::load(&tree.join("missing.toml"))?.global.toggle,
            "Ctrl+Alt+Space"
        );
        Ok(())
    }

    #[test]
    fn parses_kebab_case_keys() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file(
            "config.toml",
            "[appearance]\ntheme = \"snow-storm\"\nsize = \"l\"\n[behavior]\nhide-on-blur = false\n[status]\nmode = \"usage\"\n",
        )?;
        let config = LauncherConfig::load(&tree.join("config.toml"))?;
        assert_eq!(config.appearance.theme, ThemePreference::SnowStorm);
        assert_eq!(config.appearance.size, SizePreset::L);
        assert!(!config.behavior.hide_on_blur);
        assert!(
            config.behavior.hide_on_launch,
            "unset keys keep their default"
        );
        assert_eq!(config.status.mode, StatusMode::Usage);
        Ok(())
    }

    #[test]
    fn rejects_typos_with_the_path() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file("config.toml", "[behavior]\nhide-on-blurr = false\n")?;
        let error = LauncherConfig::load(&tree.join("config.toml"))
            .err()
            .ok_or("expected an error")?;
        assert_eq!(error.kind(), "parse");
        assert!(error.to_string().contains("config.toml"), "{error}");
        Ok(())
    }

    #[test]
    fn serialises_camel_case_for_the_ui() -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(LauncherConfig::default())?;
        assert_eq!(json["behavior"]["hideOnBlur"], true);
        assert_eq!(json["appearance"]["theme"], "system");
        assert_eq!(json["vault"]["idleLockMinutes"], 10);
        let keys = serde_json::to_value(Keybindings::default())?;
        assert_eq!(keys["launcher"]["focusSearch"], "mod+k");
        Ok(())
    }

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn settings_roundtrip_has_no_autostart_key() -> TestResult {
        let mut config = LauncherConfig::default();
        let json = serde_json::to_string(&config)?;
        assert!(!json.contains("autostart"), "{json}");

        config.appearance.theme = ThemePreference::PolarNight;
        config.appearance.size = SizePreset::L;
        config.behavior.pinned = true;
        for config in [LauncherConfig::default(), config] {
            let text = config.to_toml()?;
            assert!(!text.contains("autostart"), "{text}");
            let tree = TempTree::new()?.file("settings.toml", text.as_str())?;
            assert_eq!(LauncherConfig::load(&tree.join("settings.toml"))?, config);
        }

        // A file that still carries SlateSuite's key is reported, not silently accepted.
        let old = TempTree::new()?.file("settings.toml", "[behavior]\nautostart = true\n")?;
        let error = LauncherConfig::load(&old.join("settings.toml"))
            .err()
            .ok_or("expected an error")?;
        assert_eq!(error.kind(), "parse");
        Ok(())
    }

    #[test]
    fn invalid_settings_toml_keeps_previous_and_reports_issue() -> TestResult {
        let tree = TempTree::new()?.file(
            "settings.toml",
            "[appearance]\ntheme = \"snow-storm\"\nsize = \"l\"\n",
        )?;
        let mut settings = Settings::load(tree.path());
        assert_eq!(settings.config.appearance.theme, ThemePreference::SnowStorm);
        assert_eq!(settings.issue, None);

        tree.write("settings.toml", "[appearance]\ntheme = \n")?;
        assert!(
            settings.reload(tree.path()),
            "the issue appearing is a change"
        );
        assert_eq!(settings.config.appearance.theme, ThemePreference::SnowStorm);
        assert_eq!(settings.config.appearance.size, SizePreset::L);
        let issue = settings.issue.clone().ok_or("an issue")?;
        assert!(issue.contains("settings.toml"), "{issue}");
        assert!(issue.contains("line 2"), "{issue}");
        assert!(
            !issue.contains(tree.path().to_string_lossy().as_ref()),
            "no absolute path in the message: {issue}"
        );
        assert!(
            !settings.reload(tree.path()),
            "same broken file, nothing new"
        );

        // The other file is still read while one is broken.
        tree.write(
            "keybindings.toml",
            "[global]\ntoggle = \"Ctrl+Shift+Space\"\n",
        )?;
        settings.reload(tree.path());
        assert_eq!(settings.keybindings.global.toggle, "Ctrl+Shift+Space");
        assert!(settings.issue.is_some());

        // A wrong value (not just bad syntax) is an issue too.
        tree.write("settings.toml", "[appearance]\ntheme = \"purple\"\n")?;
        settings.reload(tree.path());
        assert_eq!(settings.config.appearance.theme, ThemePreference::SnowStorm);
        assert!(settings.issue.is_some());

        // Fixing the file clears the issue and applies the new values.
        tree.write("settings.toml", "[appearance]\ntheme = \"polar-night\"\n")?;
        assert!(settings.reload(tree.path()));
        assert_eq!(
            settings.config.appearance.theme,
            ThemePreference::PolarNight
        );
        assert_eq!(settings.issue, None);
        Ok(())
    }

    #[test]
    fn a_broken_file_on_first_load_means_defaults_plus_issue() -> TestResult {
        let tree = TempTree::new()?.file("settings.toml", "[appearance\n")?;
        let settings = Settings::load(tree.path());
        assert_eq!(settings.config, LauncherConfig::default());
        assert!(settings.issue.is_some());
        let json = serde_json::to_value(&settings)?;
        assert!(json["issue"].is_string());
        assert_eq!(json["config"]["appearance"]["theme"], "system");
        let clean = serde_json::to_value(Settings::load(TempTree::new()?.path()))?;
        assert!(clean["issue"].is_null());
        Ok(())
    }

    #[test]
    fn defaults_match_what_the_launcher_ui_expects() {
        let config = LauncherConfig::default();
        assert_eq!(config.appearance.theme, ThemePreference::System);
        assert_eq!(config.appearance.size, SizePreset::M);
        assert!(config.behavior.hide_on_blur && config.behavior.hide_on_launch);
        assert!(!config.behavior.pinned);

        let keys = Keybindings::default();
        assert_eq!(keys.global.toggle, "Ctrl+Alt+Space");
        let launcher = keys.launcher;
        assert_eq!(
            [
                launcher.focus_search.as_str(),
                launcher.toggle_tools.as_str(),
                launcher.toggle_pin.as_str(),
                launcher.toggle_favorite.as_str(),
            ],
            ["mod+k", "mod+,", "mod+p", "mod+d"]
        );
        // Tab keys follow the tab order: GENSLATE, portapps.io, PortableApps.com.
        assert_eq!(
            [
                launcher.tab_genslate.as_str(),
                launcher.tab_portapps.as_str(),
                launcher.tab_portableapps.as_str(),
            ],
            ["mod+1", "mod+2", "mod+3"]
        );
    }

    #[test]
    fn vault_lock_timers_are_settings_with_safe_defaults() -> TestResult {
        let defaults = VaultSettings::default();
        assert_eq!(
            (defaults.idle_lock_minutes, defaults.hide_lock_minutes),
            (10, 5)
        );

        let tree = TempTree::new()?.file(
            "settings.toml",
            "[vault]\nidle-lock-minutes = 0\nhide-lock-minutes = 15\n",
        )?;
        let config = LauncherConfig::load(&tree.join("settings.toml"))?;
        assert_eq!(config.vault.idle_lock_minutes, 0, "0 turns the timer off");
        assert_eq!(config.vault.hide_lock_minutes, 15);

        let negative =
            TempTree::new()?.file("settings.toml", "[vault]\nidle-lock-minutes = -1\n")?;
        assert!(LauncherConfig::load(&negative.join("settings.toml")).is_err());
        Ok(())
    }

    #[test]
    fn keybindings_roundtrip_through_toml() -> TestResult {
        let mut keys = Keybindings::default();
        keys.global.toggle = "Ctrl+Shift+L".to_owned();
        keys.launcher.tab_portableapps = String::new();
        let text = keys.to_toml()?;
        assert!(text.contains("tab-portableapps"), "{text}");
        let tree = TempTree::new()?.file("keybindings.toml", text.as_str())?;
        assert_eq!(Keybindings::load(&tree.join("keybindings.toml"))?, keys);
        Ok(())
    }
}

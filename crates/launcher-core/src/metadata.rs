//! Files in `other/launcher/configs/metadata/` (see [`metadata_dir`]):
//! - `<app>.toml`: how a GENSLATE app is shown (hand-written `[app]`, stamped `[build]`/`[exe]`).
//! - `genslate.toml`, `portapps.toml`, `portableapps.toml`: tab settings and the user's
//!   per-app overrides (favorites, hidden, names, arguments). The launcher edits these with
//!   `toml_edit`, so comments survive. They are keyed by app key (`explorer`,
//!   `FirefoxPortable`), never by path, so they keep working when the drive letter changes.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use genslate_paths::AppPaths;
use serde::{Deserialize, Serialize};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use crate::LauncherError;
use crate::catalog::Source;
use crate::config::load_toml;
use crate::error::{read_error, write_error};

/// `other/launcher/configs/metadata/`: app descriptions and per-tab settings.
pub fn metadata_dir(paths: &AppPaths) -> PathBuf {
    paths.config_dir.join("metadata")
}

/// `<app>.toml`. Unknown keys are allowed so newer metadata works with older launchers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct AppMetadata {
    pub app: AppInfo,
    pub build: BuildInfo,
    pub exe: ExePaths,
}

/// `[app]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct AppInfo {
    pub name: Option<String>,
    pub description: String,
    pub category: Option<String>,
    /// Nord colour token of the icon tile (`nord7` … `nord15`).
    pub color: Option<String>,
    pub keywords: Vec<String>,
}

/// `[build]` (stamped at package time).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct BuildInfo {
    pub guid: Option<String>,
    pub version: Option<String>,
    pub build: Option<String>,
    pub identifier: Option<String>,
}

/// `[exe]`: the launch path, relative to `programs/genslate/<app>/`. Windows is the only
/// platform; other keys (`macos`, `linux`) are ignored like any unknown key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ExePaths {
    /// The program, e.g. `slatecore-explorer.exe`.
    pub windows: Option<String>,
}

impl AppMetadata {
    /// Reads `<app>.toml`; a missing file is empty metadata.
    pub fn load(path: &Path) -> Result<Self, LauncherError> {
        load_toml(path)
    }
}

/// A tab settings file (`genslate.toml`, …).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct TabSettings {
    /// Show the tab. A tab whose folder has no apps is shown too, with a count of zero; only
    /// `false` hides it. GENSLATE's tab cannot be turned off.
    pub enabled: bool,
    /// Tab position; lower comes first.
    pub order: Option<u32>,
    /// Per-app overrides keyed by the app key (`explorer`, `FirefoxPortable`).
    pub apps: BTreeMap<String, AppOverride>,
}

impl Default for TabSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            order: None,
            apps: BTreeMap::new(),
        }
    }
}

/// `[apps.<key>]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct AppOverride {
    pub favorite: bool,
    pub hidden: bool,
    pub name: Option<String>,
    pub category: Option<String>,
    pub args: Option<Vec<String>>,
}

impl TabSettings {
    /// Reads a tab settings file; a missing file is the default.
    pub fn load(path: &Path) -> Result<Self, LauncherError> {
        load_toml(path)
    }
}

/// A change to one app's overrides. `None` leaves a field as it is; `Some(None)` removes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverridePatch {
    pub favorite: Option<bool>,
    pub hidden: Option<bool>,
    #[allow(
        clippy::option_option,
        reason = "absent = keep, null = remove, value = set"
    )]
    pub name: Option<Option<String>>,
    #[allow(
        clippy::option_option,
        reason = "absent = keep, null = remove, value = set"
    )]
    pub category: Option<Option<String>>,
    #[allow(
        clippy::option_option,
        reason = "absent = keep, null = remove, value = set"
    )]
    pub args: Option<Option<Vec<String>>>,
}

/// Applies `change` to `[apps.<key>]` of `source`'s settings file in `metadata_dir`, keeping
/// every comment and unrelated key. Defaults (`false`, removed) are deleted so the file stays
/// minimal. Returns the new file contents.
pub fn write_override(
    metadata_dir: &Path,
    source: Source,
    key: &str,
    change: &OverridePatch,
) -> Result<String, LauncherError> {
    let path = metadata_dir.join(source.settings_file());
    let mut doc = read_document(&path)?;
    let apps = doc
        .entry("apps")
        .or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        })
        .as_table_mut()
        .ok_or_else(|| parse_error(&path, "`apps` must be a table"))?;
    let entry = apps
        .entry(key)
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| parse_error(&path, &format!("`apps.{key}` must be a table")))?;

    set_flag(entry, "favorite", change.favorite);
    set_flag(entry, "hidden", change.hidden);
    set_text(entry, "name", change.name.as_ref());
    set_text(entry, "category", change.category.as_ref());
    if let Some(args) = &change.args {
        match args {
            Some(args) if !args.is_empty() => {
                set_value(entry, "args", Value::Array(args.iter().collect::<Array>()));
            }
            _ => {
                entry.remove("args");
            }
        }
    }
    if entry.is_empty() {
        apps.remove(key);
    }
    let text = doc.to_string();
    write_atomic(&path, &text)?;
    Ok(text)
}

/// Sets `[table].key = value` in a TOML file (e.g. `appearance.theme`), keeping comments.
pub fn write_value(
    path: &Path,
    table: &str,
    key: &str,
    new: Value,
) -> Result<String, LauncherError> {
    let mut doc = read_document(path)?;
    let section = doc
        .entry(table)
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| parse_error(path, &format!("`{table}` must be a table")))?;
    set_value(section, key, new);
    let text = doc.to_string();
    write_atomic(path, &text)?;
    Ok(text)
}

/// Writes `text` to a sibling temp file and renames it over `path`, so readers (and the file
/// watcher) never see a half-written file.
pub fn write_atomic(path: &Path, text: &str) -> Result<(), LauncherError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(write_error(parent))?;
    }
    let temp = path.with_extension("toml.tmp");
    fs::write(&temp, text).map_err(write_error(&temp))?;
    fs::rename(&temp, path).map_err(write_error(path))
}

fn read_document(path: &Path) -> Result<DocumentMut, LauncherError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => return Err(read_error(path)(error)),
    };
    text.parse::<DocumentMut>()
        .map_err(|error| parse_error(path, error.message()))
}

/// Sets `key` to `new` and keeps the comments around an existing value. A value that is
/// already equal is not touched at all, so re-setting it never costs a comment.
fn set_value(table: &mut Table, key: &str, new: Value) {
    match table.get_mut(key).and_then(Item::as_value_mut) {
        Some(existing) if same_value(existing, &new) => {}
        Some(existing) => {
            let decor = existing.decor().clone();
            *existing = new;
            *existing.decor_mut() = decor;
        }
        None => {
            table.insert(key, Item::Value(new));
        }
    }
}

/// Whether two values mean the same, whatever their spacing and comments.
fn same_value(a: &Value, b: &Value) -> bool {
    if let (Some(a), Some(b)) = (a.as_bool(), b.as_bool()) {
        return a == b;
    }
    if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
        return a == b;
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        return a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_value(a, b));
    }
    false
}

fn set_flag(table: &mut Table, key: &str, flag: Option<bool>) {
    match flag {
        Some(true) => set_value(table, key, Value::from(true)),
        Some(false) => {
            table.remove(key);
        }
        None => {}
    }
}

fn set_text(table: &mut Table, key: &str, text: Option<&Option<String>>) {
    match text {
        Some(Some(text)) if !text.trim().is_empty() => {
            set_value(table, key, Value::from(text.trim()));
        }
        Some(_) => {
            table.remove(key);
        }
        None => {}
    }
}

fn parse_error(path: &Path, message: &str) -> LauncherError {
    LauncherError::Parse {
        path: path.to_path_buf(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_testing::TempTree;

    const SETTINGS: &str = "# GENSLATE tab — keep this comment\nenabled = true\n\n[apps.explorer]\n# my favourite\nfavorite = true\n";

    #[test]
    fn reads_metadata_and_settings() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?
            .file(
                "explorer.toml",
                "[app]\nname = \"Explorer\"\ndescription = \"Files\"\ncolor = \"nord8\"\nfuture-key = 1\n[exe]\nwindows = \"x.exe\"\n",
            )?
            .file("genslate.toml", SETTINGS)?;
        let meta = AppMetadata::load(&tree.join("explorer.toml"))?;
        assert_eq!(meta.app.name.as_deref(), Some("Explorer"));
        assert_eq!(meta.exe.windows.as_deref(), Some("x.exe"));
        let tab = TabSettings::load(&tree.join("genslate.toml"))?;
        assert!(tab.apps.get("explorer").is_some_and(|o| o.favorite));
        Ok(())
    }

    #[test]
    fn overrides_keep_comments_and_prune_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file("genslate.toml", SETTINGS)?;
        let patch = OverridePatch {
            hidden: Some(true),
            name: Some(Some("Files".to_owned())),
            ..OverridePatch::default()
        };
        let text = write_override(tree.path(), Source::Genslate, "explorer", &patch)?;
        assert!(
            text.contains("# GENSLATE tab — keep this comment"),
            "{text}"
        );
        assert!(text.contains("# my favourite"), "{text}");
        let tab = TabSettings::load(&tree.join("genslate.toml"))?;
        let explorer = tab.apps.get("explorer").ok_or("missing override")?;
        assert!(explorer.favorite && explorer.hidden);
        assert_eq!(explorer.name.as_deref(), Some("Files"));

        let reset = OverridePatch {
            favorite: Some(false),
            hidden: Some(false),
            name: Some(None),
            ..OverridePatch::default()
        };
        write_override(tree.path(), Source::Genslate, "explorer", &reset)?;
        let tab = TabSettings::load(&tree.join("genslate.toml"))?;
        assert!(
            !tab.apps.contains_key("explorer"),
            "empty override tables are removed"
        );
        Ok(())
    }

    #[test]
    fn overrides_create_the_file() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?;
        let patch = OverridePatch {
            favorite: Some(true),
            args: Some(Some(vec!["--private".to_owned()])),
            ..OverridePatch::default()
        };
        write_override(tree.path(), Source::PortableApps, "FirefoxPortable", &patch)?;
        let tab = TabSettings::load(&tree.join("portableapps.toml"))?;
        let firefox = tab.apps.get("FirefoxPortable").ok_or("missing override")?;
        assert_eq!(firefox.args.as_deref(), Some(&["--private".to_owned()][..]));
        Ok(())
    }

    #[test]
    fn values_keep_their_trailing_comments() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file(
            "config.toml",
            "[appearance]\n# pick one\ntheme = \"system\" # or a Nord theme\n",
        )?;
        let text = write_value(
            &tree.join("config.toml"),
            "appearance",
            "theme",
            "snow-storm".into(),
        )?;
        assert!(
            text.contains("theme = \"snow-storm\" # or a Nord theme"),
            "{text}"
        );
        assert!(text.contains("# pick one"));
        Ok(())
    }

    #[test]
    fn exe_keys_for_other_platforms_are_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file(
            "tool.toml",
            "[exe]\nwindows = \"tool.exe\"\nmacos = \"Tool.app\"\nlinux = \"tool\"\n",
        )?;
        let meta = AppMetadata::load(&tree.join("tool.toml"))?;
        assert_eq!(meta.exe.windows.as_deref(), Some("tool.exe"));
        Ok(())
    }

    #[test]
    fn the_seed_settings_in_the_repo_are_valid() {
        // `programs/desktop/launcher/other/launcher/configs/` seeds `installDir/other/…`.
        let seed = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../programs/desktop/launcher/other/launcher/configs");
        if seed.is_dir() {
            let settings = crate::config::Settings::load(&seed);
            assert_eq!(settings.issue, None);
        }
    }

    #[test]
    fn setting_a_value_again_keeps_its_trailing_comment() -> Result<(), Box<dyn std::error::Error>>
    {
        let original = "[apps.explorer]\nfavorite = true # my favourite\nname = \"Files\" # shown name\nargs = [\"--new\"] # extra\n";
        let tree = TempTree::new()?.file("genslate.toml", original)?;
        let same = OverridePatch {
            favorite: Some(true),
            name: Some(Some("Files".to_owned())),
            args: Some(Some(vec!["--new".to_owned()])),
            ..OverridePatch::default()
        };
        let text = write_override(tree.path(), Source::Genslate, "explorer", &same)?;
        assert_eq!(text, original, "equal values are left exactly as they were");

        // A different value replaces the value only: the comment stays.
        let changed = OverridePatch {
            name: Some(Some("Documents".to_owned())),
            args: Some(Some(vec!["--old".to_owned()])),
            ..OverridePatch::default()
        };
        let text = write_override(tree.path(), Source::Genslate, "explorer", &changed)?;
        assert!(text.contains("favorite = true # my favourite"), "{text}");
        assert!(text.contains("name = \"Documents\" # shown name"), "{text}");
        assert!(text.contains("# extra"), "{text}");
        let tab = TabSettings::load(&tree.join("genslate.toml"))?;
        let explorer = tab.apps.get("explorer").ok_or("explorer")?;
        assert_eq!(explorer.name.as_deref(), Some("Documents"));
        assert_eq!(explorer.args.as_deref(), Some(&["--old".to_owned()][..]));
        Ok(())
    }

    #[test]
    fn write_value_leaves_an_equal_value_alone() -> Result<(), Box<dyn std::error::Error>> {
        let original = "[appearance]\ntheme = \"system\"   # keep my spacing\n";
        let tree = TempTree::new()?.file("settings.toml", original)?;
        let text = write_value(
            &tree.join("settings.toml"),
            "appearance",
            "theme",
            "system".into(),
        )?;
        assert_eq!(text, original);
        Ok(())
    }
}

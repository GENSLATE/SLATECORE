//! portapps.io apps in `programs/portapps.io/`: `<id>/portapp.json` (`id`, `name`, `version`,
//! `publisher`) next to `<id>.exe`. The icon is embedded in the exe.
//! See <https://portapps.io/doc/configuration/>.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;

use super::genslate::list;
use super::icon::IconSource;
use super::model::{AppEntry, AppId, AppStatus, Source};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PortappJson {
    id: String,
    name: String,
    version: String,
    publisher: String,
}

/// Category for every portapp (portapp.json has none).
const CATEGORY: &str = "Apps";

/// Scans `dir` (`programs/portapps.io/`).
pub fn scan(dir: &Path) -> Vec<AppEntry> {
    list(dir)
        .into_iter()
        .filter(|path| path.is_dir())
        .filter_map(|package| entry(&package))
        .collect()
}

/// `None` for a folder without a `portapp.json` (not an app). A manifest that cannot be read
/// or parsed gives a [`AppStatus::BrokenManifest`] entry, a valid one without its program a
/// [`AppStatus::MissingExe`] entry.
fn entry(package: &Path) -> Option<AppEntry> {
    let folder = package.file_name()?.to_str()?;
    let bytes = match fs::read(package.join("portapp.json")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(error) => {
            log::warn!("{}: {error}; listing it as unavailable", package.display());
            return Some(unavailable(package, folder, AppStatus::BrokenManifest));
        }
    };
    let Some(json) = parse_manifest(&bytes) else {
        log::warn!(
            "{}: portapp.json is not valid; listing it as unavailable",
            package.display()
        );
        return Some(unavailable(package, folder, AppStatus::BrokenManifest));
    };
    let name = if json.name.trim().is_empty() {
        display_name(folder)
    } else {
        json.name.trim().to_owned()
    };
    let mut app = AppEntry::new(AppId::new(Source::Portapps, folder), name);
    CATEGORY.clone_into(&mut app.category);
    app.version = non_empty(json.version);
    app.publisher = non_empty(json.publisher);
    app.description = format!("{} (portapps.io)", app.name);
    app.dir = Some(package.to_path_buf());
    match exe_for(package, folder, &json.id) {
        Some(exe) => {
            app.icon = Some(IconSource::Executable(exe.clone()));
            app.has_icon = true;
            app.program = Some(exe);
        }
        None => app.status = AppStatus::MissingExe,
    }
    Some(app)
}

/// `portapp.json` must be a JSON object whose known keys have the right types.
fn parse_manifest(bytes: &[u8]) -> Option<PortappJson> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if !value.is_object() {
        return None;
    }
    serde_json::from_value(value).ok()
}

fn unavailable(package: &Path, folder: &str, status: AppStatus) -> AppEntry {
    let mut app = AppEntry::new(AppId::new(Source::Portapps, folder), display_name(folder));
    CATEGORY.clone_into(&mut app.category);
    app.status = status;
    app.dir = Some(package.to_path_buf());
    app
}

/// `brave-portable` → `brave`.
fn display_name(folder: &str) -> String {
    folder.trim_end_matches("-portable").to_owned()
}

/// `<id>.exe`, else `<folder>.exe`, else the only `*-portable.exe` in the folder.
fn exe_for(package: &Path, folder: &str, id: &str) -> Option<std::path::PathBuf> {
    let valid_id = !id.is_empty() && !id.contains(['/', '\\', '.']);
    let named = valid_id
        .then(|| package.join(format!("{id}.exe")))
        .into_iter()
        .chain([package.join(format!("{folder}.exe"))])
        .find(|path| path.is_file());
    named.or_else(|| {
        let mut candidates = list(package).into_iter().filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.to_ascii_lowercase().ends_with("-portable.exe"))
        });
        let first = candidates.next();
        candidates.next().is_none().then_some(first).flatten()
    })
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_testing::TempTree;

    #[test]
    fn reads_portapp_json() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?
            .file(
                "brave-portable/portapp.json",
                r#"{"id":"brave-portable","guid":"x","name":"Brave","version":"128.0-1","publisher":"Portapps","portapps_version":"3.10"}"#,
            )?
            .file("brave-portable/brave-portable.exe", "")?
            .file("broken/portapp.json", "{")?
            .file("noexe/portapp.json", r#"{"id":"noexe"}"#)?;
        let apps = scan(tree.path());
        let ids: Vec<&str> = apps.iter().map(|a| a.id.key.as_str()).collect();
        assert_eq!(ids, ["brave-portable", "broken", "noexe"]);
        let brave = &apps[0];
        assert_eq!(brave.id.to_string(), "portapps/brave-portable");
        assert_eq!(brave.name, "Brave");
        assert_eq!(brave.version.as_deref(), Some("128.0-1"));
        assert!(matches!(brave.icon, Some(IconSource::Executable(_))));
        assert_eq!(brave.status, AppStatus::Ready);
        assert_eq!(apps[1].status, AppStatus::BrokenManifest);
        assert_eq!(apps[2].status, AppStatus::MissingExe);
        assert!(apps[2].program.is_none(), "nothing to launch");
        Ok(())
    }

    #[test]
    fn malformed_portapps_manifest_goes_to_unavailable() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?
            .file("discord-portable/portapp.json", "{")?
            .file("discord-portable/discord-portable.exe", "")?
            .file("empty-portable/portapp.json", "")?
            .file("typed-portable/portapp.json", r#"{"id":5}"#)?
            .file("list-portable/portapp.json", "[]")?
            .file(
                "brave-portable/portapp.json",
                r#"{"id":"brave-portable","name":"Brave"}"#,
            )?
            .file("brave-portable/brave-portable.exe", "")?
            .file("stray/readme.txt", "not an app")?;
        let apps = scan(tree.path());
        let status = |key: &str| apps.iter().find(|a| a.id.key == key).map(|a| a.status);
        for broken in [
            "discord-portable",
            "empty-portable",
            "typed-portable",
            "list-portable",
        ] {
            assert_eq!(status(broken), Some(AppStatus::BrokenManifest), "{broken}");
        }
        assert_eq!(status("brave-portable"), Some(AppStatus::Ready));
        assert_eq!(
            status("stray"),
            None,
            "folders without a manifest are not apps"
        );
        assert!(
            apps.iter()
                .filter(|a| a.status == AppStatus::BrokenManifest)
                .all(|a| a.program.is_none()),
            "a broken app is never launchable"
        );
        Ok(())
    }
}

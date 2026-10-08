//! portapps.io apps in `programs/portapps.io/`: `<id>/portapp.json` (`id`, `name`, `version`,
//! `publisher`) next to `<id>.exe`. The icon is embedded in the exe.
//! See <https://portapps.io/doc/configuration/>.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::genslate::{has_files, is_relative_inside, list};
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

/// A manifest that cannot be read or parsed gives a [`AppStatus::BrokenManifest`] entry, a
/// valid one without its program a [`AppStatus::MissingExe`] entry. A folder without a
/// manifest is handled by [`without_manifest`]; only an empty or hidden folder gives `None`.
fn entry(package: &Path) -> Option<AppEntry> {
    let folder = package.file_name()?.to_str()?;
    let bytes = match fs::read(package.join("portapp.json")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return without_manifest(package, folder);
        }
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

/// A folder in `programs/portapps.io/` without a `portapp.json`: still shown, because the user
/// put it there. It is a normal app when its program can be told apart (`<folder>.exe`, the
/// only `*-portable.exe`, or the only `.exe`), else it goes to "Unavailable". Empty folders
/// (a `.gitkeep` does not count) and hidden ones (`.git`) are not apps and give `None`.
fn without_manifest(package: &Path, folder: &str) -> Option<AppEntry> {
    if folder.starts_with('.') || !has_files(package) {
        return None;
    }
    let Some(exe) = exe_for(package, folder, "").or_else(|| only_exe(package)) else {
        log::warn!(
            "{}: no portapp.json and no program to tell apart; listing it as unavailable",
            package.display()
        );
        return Some(unavailable(package, folder, AppStatus::BrokenManifest));
    };
    let mut app = AppEntry::new(AppId::new(Source::Portapps, folder), display_name(folder));
    CATEGORY.clone_into(&mut app.category);
    app.description = format!("{} (portapps.io)", app.name);
    app.dir = Some(package.to_path_buf());
    app.icon = Some(IconSource::Executable(exe.clone()));
    app.has_icon = true;
    app.program = Some(exe);
    Some(app)
}

/// The only `.exe` directly in `package`, if there is exactly one.
fn only_exe(package: &Path) -> Option<PathBuf> {
    let mut exes = list(package).into_iter().filter(|path| {
        path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    });
    let first = exes.next();
    exes.next().is_none().then_some(first).flatten()
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
fn exe_for(package: &Path, folder: &str, id: &str) -> Option<PathBuf> {
    let named = valid_id(id)
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

/// An `id` from the manifest becomes a file name, so it must be one plain name: no separators,
/// dots, drive letters (`C:x` is drive-relative on Windows) or other path syntax.
fn valid_id(id: &str) -> bool {
    !id.contains(['/', '\\', '.']) && is_relative_inside(id)
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
            Some(AppStatus::BrokenManifest),
            "a folder with files but no manifest and no program is unavailable, not dropped"
        );
        assert!(
            apps.iter()
                .filter(|a| a.status == AppStatus::BrokenManifest)
                .all(|a| a.program.is_none()),
            "a broken app is never launchable"
        );
        Ok(())
    }

    #[test]
    fn folders_without_a_manifest_fall_back_to_their_program_or_go_to_unavailable()
    -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?
            .file("named-portable/named-portable.exe", "")?
            .file("suffix/tool-portable.exe", "")?
            .file("suffix/unins000.exe", "")?
            .file("lone/Setup.exe", "")?
            .file("lone/readme.txt", "")?
            .file("two/a.exe", "")?
            .file("two/b.exe", "")?
            .file("stray/readme.txt", "")?
            .file("empty/.gitkeep", "")?
            .file(".git/config", "")?
            .file(".hidden-portable/hidden-portable.exe", "")?;
        let apps = scan(tree.path());
        let found: Vec<(&str, AppStatus)> =
            apps.iter().map(|a| (a.id.key.as_str(), a.status)).collect();
        assert_eq!(
            found,
            [
                ("lone", AppStatus::Ready),
                ("named-portable", AppStatus::Ready),
                ("stray", AppStatus::BrokenManifest),
                ("suffix", AppStatus::Ready),
                ("two", AppStatus::BrokenManifest),
            ],
            "empty and hidden folders are skipped, the rest is listed"
        );
        let program = |key: &str| {
            apps.iter()
                .find(|a| a.id.key == key)
                .and_then(|a| a.program.clone())
        };
        assert!(program("lone").is_some_and(|p| p.ends_with("Setup.exe")));
        assert!(program("suffix").is_some_and(|p| p.ends_with("tool-portable.exe")));
        assert!(program("named-portable").is_some());
        assert_eq!(program("two"), None, "two programs: it is not guessed");
        assert_eq!(program("stray"), None);
        let named = apps
            .iter()
            .find(|a| a.id.key == "named-portable")
            .ok_or("named")?;
        assert_eq!(named.name, "named");
        assert!(named.has_icon && matches!(named.icon, Some(IconSource::Executable(_))));
        Ok(())
    }

    #[test]
    fn manifest_ids_are_single_plain_names() {
        assert!(valid_id("brave-portable"));
        assert!(valid_id("7zip"));
        for bad in [
            "",
            "a/b",
            "a\\b",
            "a.b",
            "..",
            "C:evil",
            "C:\\evil",
            "a:b",
            "\\\\srv\\x",
        ] {
            assert!(!valid_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_manifest_id_with_a_drive_never_selects_a_program() -> Result<(), Box<dyn std::error::Error>>
    {
        let tree = TempTree::new()?
            .file("x-portable/portapp.json", r#"{"id":"C:evil"}"#)?
            .file("x-portable/x-portable.exe", "")?;
        let apps = scan(tree.path());
        let program = apps
            .first()
            .and_then(|a| a.program.clone())
            .ok_or("program")?;
        assert!(program.ends_with("x-portable.exe"), "{program:?}");
        Ok(())
    }
}

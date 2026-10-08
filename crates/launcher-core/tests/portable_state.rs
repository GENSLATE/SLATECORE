//! The drive letter changes between PCs (`S:` on one, `E:` on the next), so the launcher's
//! persisted state must be relative: ids and settings, never an absolute path.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use genslate_launcher_core::catalog::{AppId, Catalog, CatalogRoots, Source};
use genslate_launcher_core::config::{SETTINGS_FILE, Settings, SizePreset, ThemePreference};
use genslate_launcher_core::metadata::{OverridePatch, metadata_dir, write_override, write_value};
use genslate_launcher_core::recent::{RecentLaunches, recent_path};
use genslate_paths::{AppPaths, Environment, resolve_with};
use genslate_testing::{TempTree, fake_suite};

type TestResult = Result<(), Box<dyn Error>>;

const APPINFO: &str =
    "[Details]\nName=Mozilla Firefox\nCategory=Internet\n\n[Control]\nStart=FirefoxPortable.exe\n";

/// Builds an install folder at `<tmp>/<drive>/<dir>/SLATECORE` with one app per source.
fn install(tmp: &TempTree, drive_and_dir: &str) -> Result<(PathBuf, AppPaths), Box<dyn Error>> {
    let root = fake_suite(tmp.join(drive_and_dir))?;
    for (file, contents) in [
        ("programs/genslate/explorer/slatecore-explorer.exe", ""),
        ("programs/portapps.io/brave-portable/brave-portable.exe", ""),
        (
            "programs/portapps.io/brave-portable/portapp.json",
            r#"{"id":"brave-portable","name":"Brave"}"#,
        ),
        (
            "programs/portableapps.com/FirefoxPortable/FirefoxPortable.exe",
            "",
        ),
        (
            "programs/portableapps.com/FirefoxPortable/App/AppInfo/appinfo.ini",
            APPINFO,
        ),
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().ok_or("parent")?)?;
        fs::write(path, contents)?;
    }
    let env = Environment {
        exe: Some(root.join("programs/genslate/launcher/slatecore-launcher.exe")),
        ..Environment::default()
    };
    let paths = resolve_with("launcher", &env)?;
    paths.create_dirs()?;
    Ok((root, paths))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn files_below(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(files_below(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

/// `X:\` or `X:/` where `X` stands alone (so `https://` does not count).
fn has_drive_path(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    chars.windows(3).enumerate().any(|(index, window)| {
        let [letter, colon, slash] = *window else {
            return false;
        };
        let alone = index == 0 || !chars[index - 1].is_alphanumeric();
        letter.is_ascii_alphabetic() && colon == ':' && matches!(slash, '\\' | '/') && alone
    })
}

/// Everything the launcher persists: settings, favorites and the recent list.
fn persist_state(paths: &AppPaths) -> TestResult {
    let settings_file = paths.config_dir.join(SETTINGS_FILE);
    write_value(&settings_file, "appearance", "theme", "snow-storm".into())?;
    write_value(&settings_file, "appearance", "size", "l".into())?;
    let metadata_dir = metadata_dir(paths);
    let favorite = OverridePatch {
        favorite: Some(true),
        args: Some(Some(vec!["--private".to_owned()])),
        ..OverridePatch::default()
    };
    write_override(&metadata_dir, Source::Genslate, "explorer", &favorite)?;
    write_override(&metadata_dir, Source::Portapps, "brave-portable", &favorite)?;
    let recent_file = recent_path(paths);
    let mut recent = RecentLaunches::load(&recent_file);
    recent.record(
        AppId::new(Source::PortableApps, "FirefoxPortable"),
        1,
        &recent_file,
    )?;
    recent.record(AppId::new(Source::Genslate, "explorer"), 2, &recent_file)?;
    Ok(())
}

/// No persisted file mentions the install folder, a drive letter or the test folder.
fn assert_no_absolute_paths(paths: &AppPaths, root: &Path, tmp: &TempTree) -> TestResult {
    let root_text = root.to_string_lossy().into_owned();
    let mut persisted = files_below(&paths.config_dir)?;
    persisted.extend(files_below(&paths.database_dir)?);
    assert!(persisted.len() >= 4, "{persisted:?}");
    for file in persisted {
        let text = fs::read_to_string(&file)?;
        for spelling in [root_text.clone(), root_text.replace('/', "\\")] {
            assert!(
                !text.contains(&spelling),
                "{} holds the root path",
                file.display()
            );
        }
        assert!(
            !has_drive_path(&text),
            "{} holds a drive path:\n{text}",
            file.display()
        );
        assert!(
            !text.contains(tmp.path().to_string_lossy().as_ref()),
            "{} holds a path into the test directory: {text}",
            file.display()
        );
    }
    Ok(())
}

fn app_ids(catalog: &Catalog) -> Vec<String> {
    catalog
        .apps()
        .iter()
        .map(|app| app.id.to_string())
        .collect()
}

#[test]
fn persisted_state_contains_no_absolute_paths() -> TestResult {
    let tmp = TempTree::new()?;
    let (root_a, paths_a) = install(&tmp, "S/a")?;
    persist_state(&paths_a)?;
    assert_no_absolute_paths(&paths_a, &root_a, &tmp)?;

    // Move the state to another drive (S:\a becomes E:\b) and read it back there.
    let (root_b, paths_b) = install(&tmp, "E/b")?;
    assert_ne!(root_a, root_b);
    copy_dir(&paths_a.config_dir, &paths_b.config_dir)?;
    copy_dir(&paths_a.database_dir, &paths_b.database_dir)?;

    let settings = Settings::load(&paths_b.config_dir);
    assert_eq!(settings.issue, None);
    assert_eq!(settings.config.appearance.theme, ThemePreference::SnowStorm);
    assert_eq!(settings.config.appearance.size, SizePreset::L);

    let before = Catalog::scan(&CatalogRoots::from_paths(&paths_a));
    let after = Catalog::scan(&CatalogRoots::from_paths(&paths_b));
    assert_eq!(app_ids(&before), app_ids(&after));
    assert_eq!(
        app_ids(&after),
        [
            "genslate/explorer",
            "portapps/brave-portable",
            "portableapps/FirefoxPortable"
        ]
    );

    let favorites: BTreeSet<String> = after
        .apps()
        .iter()
        .filter(|app| app.favorite)
        .map(|app| app.id.to_string())
        .collect();
    assert_eq!(
        favorites,
        BTreeSet::from([
            "genslate/explorer".to_owned(),
            "portapps/brave-portable".to_owned()
        ])
    );
    let explorer = after
        .find(&AppId::new(Source::Genslate, "explorer"))
        .ok_or("explorer")?;
    assert_eq!(explorer.args, ["--private"]);

    let latest = RecentLaunches::load(&recent_path(&paths_b)).latest(5);
    assert_eq!(
        latest,
        [
            AppId::new(Source::Genslate, "explorer"),
            AppId::new(Source::PortableApps, "FirefoxPortable")
        ]
    );
    for id in latest {
        let app = after
            .find(&id)
            .ok_or("recent app resolves on the new drive")?;
        let program = app.program.as_ref().ok_or("program")?;
        assert!(
            program.starts_with(&root_b),
            "{program:?} is not on the new drive"
        );
        assert!(!program.starts_with(&root_a));
    }
    Ok(())
}

#[test]
fn drive_paths_are_recognised_by_the_checker() {
    assert!(has_drive_path(r"theme = 'S:\a\SLATECORE'"));
    assert!(has_drive_path("path = \"E:/b\""));
    assert!(!has_drive_path("url = \"https://example.org\""));
    assert!(!has_drive_path("id = \"genslate/explorer\""));
}

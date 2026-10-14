//! The log is persisted in the install folder and travels with the drive, so it must never
//! carry a drive letter or a user folder: paths are written relative to `programs/`, or as a
//! file name. These tests capture what the crate logs (and the issue texts it hands to the UI)
//! and check that.

use std::path::Path;
use std::sync::{Mutex, Once, PoisonError};
use std::thread::{self, ThreadId};
use std::time::Duration;

use genslate_testing::TempTree;

use crate::catalog::{Catalog, CatalogRoots};
use crate::config::Settings;
use crate::recent::RecentLaunches;
use crate::watch::{WatchTarget, watch};

static RECORDS: Mutex<Vec<(ThreadId, String)>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    /// Only this crate's own records; dependencies (the file watcher's poller) are noisy.
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target().starts_with("genslate_launcher_core")
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        RECORDS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((
                thread::current().id(),
                format!("{} {}", record.level(), record.args()),
            ));
    }

    fn flush(&self) {}
}

/// Runs `run` and returns what the calling thread logged meanwhile ("LEVEL message").
/// Tests run in parallel; each only sees its own thread's records.
pub(crate) fn capture<R>(run: impl FnOnce() -> R) -> (R, Vec<String>) {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        // Another logger being installed first only makes the assertions below fail loudly.
        if log::set_logger(&Capture).is_ok() {
            log::set_max_level(log::LevelFilter::Trace);
        }
    });
    let me = thread::current().id();
    let take = || {
        let mut records = RECORDS.lock().unwrap_or_else(PoisonError::into_inner);
        let (mine, others): (Vec<_>, Vec<_>) = records.drain(..).partition(|(id, _)| *id == me);
        *records = others;
        mine.into_iter().map(|(_, line)| line).collect::<Vec<_>>()
    };
    take();
    let result = run();
    (result, take())
}

/// `true` if `text` contains `X:\` or `X:/`.
fn has_drive_path(text: &str) -> bool {
    text.as_bytes()
        .windows(3)
        .any(|w| w[0].is_ascii_alphabetic() && w[1] == b':' && matches!(w[2], b'\\' | b'/'))
}

/// Fails if any text names `root` (the temp install), a drive path or a UNC path.
pub(crate) fn assert_no_paths(texts: &[String], root: &Path) {
    let root = root.to_string_lossy();
    let root_slashes = root.replace('\\', "/");
    for text in texts {
        assert!(
            !text.contains(root.as_ref())
                && !text.replace('\\', "/").contains(&root_slashes)
                && !has_drive_path(text)
                && !text.contains("\\\\"),
            "absolute path in {text:?}"
        );
    }
}

fn roots(tree: &TempTree) -> CatalogRoots {
    CatalogRoots {
        programs: Some(tree.join("programs")),
        metadata: tree.join("metadata"),
        icons: tree.join("icons"),
        dev_target: None,
    }
}

#[test]
fn the_helper_recognises_absolute_paths() {
    assert!(has_drive_path("could not read S:\\SLATECORE\\x"));
    assert!(has_drive_path("E:/x"));
    assert!(!has_drive_path("portapps.io/brave-portable: invalid"));
    assert!(!has_drive_path("line 3: expected `=`"));
}

#[test]
fn scanning_broken_apps_logs_and_reports_relative_names_only()
-> Result<(), Box<dyn std::error::Error>> {
    let tree = TempTree::new()?
        // portapps.io: a manifest that is not JSON, and a folder with two exes and no manifest.
        .file("programs/portapps.io/bad-portable/portapp.json", "{not json")?
        .file("programs/portapps.io/mix/a.exe", "")?
        .file("programs/portapps.io/mix/b.exe", "")?
        // PortableApps.com: a manifest that is no appinfo.ini.
        .file("programs/portableapps.com/BrokenPortable/App/AppInfo/appinfo.ini", "no sections here\n[")?
        // GENSLATE: metadata that does not parse, and a tab file that does not parse.
        .file("programs/genslate/odd/readme.txt", "x")?
        .file("metadata/odd.toml", "[app\nname = ")?
        .file("metadata/portapps.toml", "favorites = [")?;
    let (catalog, lines) = capture(|| Catalog::scan(&roots(&tree)));

    // Every problem was logged, by a name below programs/ or by file name.
    let log = lines.join("\n");
    for expected in [
        "portapps.io/bad-portable",
        "portapps.io/mix",
        "portableapps.com/BrokenPortable",
        "odd.toml",
        "portapps.toml",
    ] {
        assert!(log.contains(expected), "{expected} missing from:\n{log}");
    }
    assert_no_paths(&lines, tree.path());
    // The texts for the UI follow the same rule.
    assert_eq!(catalog.issues().len(), 1, "{:?}", catalog.issues());
    assert_no_paths(catalog.issues(), tree.path());
    Ok(())
}

#[test]
fn invalid_settings_files_are_logged_and_reported_by_file_name()
-> Result<(), Box<dyn std::error::Error>> {
    let tree = TempTree::new()?
        .file("config/settings.toml", "[appearance\n")?
        .file("config/keybindings.toml", "global = 3\n")?;
    let (settings, lines) = capture(|| Settings::load(&tree.join("config")));
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_no_paths(&lines, tree.path());
    let issue = settings.issue.ok_or("an issue")?;
    assert!(issue.contains("settings.toml") && issue.contains("keybindings.toml"));
    assert_no_paths(&[issue], tree.path());
    Ok(())
}

#[test]
fn a_broken_launch_history_is_logged_by_file_name() -> Result<(), Box<dyn std::error::Error>> {
    let tree = TempTree::new()?.file("db/recent.toml", "launch = [")?;
    let (_, lines) = capture(|| RecentLaunches::load(&tree.join("db/recent.toml")));
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("recent.toml"));
    assert_no_paths(&lines, tree.path());
    Ok(())
}

#[test]
fn a_folder_that_cannot_be_watched_is_logged_by_name_only() -> Result<(), Box<dyn std::error::Error>>
{
    let tree = TempTree::new()?;
    let target = WatchTarget {
        path: tree.join("not/there/metadata"),
        recursive: false,
    };
    let (watcher, lines) = capture(|| watch(&[target], Duration::from_millis(10), |_| {}));
    let _watcher = watcher?;
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("metadata"));
    assert_no_paths(&lines, tree.path());
    Ok(())
}

#[test]
fn issue_texts_never_carry_folders() {
    use crate::LauncherError::{OutsideRoot, Parse, Read, Spawn, Write};
    let io = || std::io::Error::other("denied");
    let errors = [
        Read {
            path: "S:\\SLATECORE\\other\\launcher\\configs\\settings.toml".into(),
            source: io(),
        },
        Write {
            path: "S:\\SLATECORE\\other\\launcher\\configs\\settings.toml".into(),
            source: io(),
        },
        Parse {
            path: "S:\\SLATECORE\\other\\x.toml".into(),
            message: "line 1".into(),
        },
        OutsideRoot("S:\\Elsewhere\\x.exe".into()),
        Spawn {
            path: "S:\\SLATECORE\\programs\\genslate\\x\\x.exe".into(),
            source: io(),
        },
    ];
    let texts: Vec<String> = errors.iter().map(crate::config::issue_text).collect();
    for text in &texts {
        assert!(
            !has_drive_path(text) && !text.contains("SLATECORE\\"),
            "{text}"
        );
    }
    assert!(texts[4].contains("genslate/x/x.exe"), "{}", texts[4]);
}

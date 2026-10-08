//! The `WebView2` data folder is disposable (windows are incognito), and a profile written by a
//! newer runtime may not open with an older one. So installDir remembers which runtime wrote the
//! folder (`cache/webview2.stamp`, `host:<version>` or `bundled:<folder>`) and empties the
//! folder when another runtime is about to use it.

use std::fs;
use std::io;
use std::path::Path;

use genslate_paths::AppPaths;

use crate::runtime::{Runtime, Version};

/// File name of the stamp, inside the launcher's cache folder.
pub const STAMP_FILE: &str = "webview2.stamp";

/// The stamp text for a runtime: `host:154.0.4258.62` or `bundled:154.0.4258.62-x64`.
pub fn stamp_text(runtime: &Runtime, host: Option<Version>) -> String {
    match runtime {
        Runtime::Host => format!(
            "host:{}",
            host.map_or_else(|| "unknown".to_owned(), |v| v.to_string())
        ),
        Runtime::Bundled(dir) => format!(
            "bundled:{}",
            dir.file_name()
                .map_or_else(|| "unknown".into(), |n| n.to_string_lossy())
        ),
    }
}

/// `true` when the folder was last used by a different runtime, or by an unknown one while it
/// holds data.
pub fn needs_reset(previous: Option<&str>, current: &str, has_data: bool) -> bool {
    match previous {
        Some(previous) => previous.trim() != current,
        None => has_data,
    }
}

/// Empties the `WebView2` data folder when `current` differs from the stamp, then writes the
/// stamp. Returns whether the folder was reset.
///
/// The folder is only ever emptied when it lies strictly inside the launcher's cache folder.
///
/// # Errors
///
/// I/O errors from removing the folder or writing the stamp.
pub fn reset_if_runtime_changed(paths: &AppPaths, current: &str) -> io::Result<bool> {
    let dir = &paths.webview2_dir;
    let stamp = paths.cache_dir.join(STAMP_FILE);
    let previous = fs::read_to_string(&stamp).ok();
    let has_data = fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some());
    let reset = needs_reset(previous.as_deref(), current, has_data);
    if reset {
        if !(dir.starts_with(&paths.cache_dir) && dir != &paths.cache_dir) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the WebView2 data folder is not inside the cache folder",
            ));
        }
        match fs::remove_dir_all(dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        fs::create_dir_all(dir)?;
    }
    if previous.as_deref().map(str::trim) != Some(current) {
        write_stamp(&stamp, current)?;
    }
    Ok(reset)
}

fn write_stamp(stamp: &Path, text: &str) -> io::Result<()> {
    let temp = stamp.with_extension("stamp.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, stamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_paths::{Environment, resolve_with};
    use genslate_testing::{TempTree, fake_suite};
    use std::error::Error;
    use std::path::PathBuf;

    type TestResult = Result<(), Box<dyn Error>>;

    fn paths_in(tmp: &TempTree) -> Result<AppPaths, Box<dyn Error>> {
        let root = fake_suite(tmp.path())?;
        let env = Environment {
            install_dir_override: Some(root),
            ..Environment::default()
        };
        let paths = resolve_with("launcher", &env)?;
        paths.create_dirs()?;
        Ok(paths)
    }

    #[test]
    fn stamps_name_the_source_and_version() {
        let host = Some(Version::new(154, 0, 4258, 62));
        assert_eq!(stamp_text(&Runtime::Host, host), "host:154.0.4258.62");
        assert_eq!(stamp_text(&Runtime::Host, None), "host:unknown");
        let bundled = Runtime::Bundled(PathBuf::from("x/webview2/153.0.1.1-x64"));
        assert_eq!(stamp_text(&bundled, host), "bundled:153.0.1.1-x64");
    }

    #[test]
    fn a_fresh_install_is_not_reset_but_gets_a_stamp() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        assert!(!reset_if_runtime_changed(&paths, "host:154.0.0.1")?);
        assert_eq!(
            fs::read_to_string(paths.cache_dir.join(STAMP_FILE))?,
            "host:154.0.0.1"
        );
        Ok(())
    }

    #[test]
    fn the_folder_is_emptied_only_when_the_runtime_changed() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        reset_if_runtime_changed(&paths, "host:154.0.0.1")?;
        fs::create_dir_all(paths.webview2_dir.join("EBWebView"))?;
        fs::write(paths.webview2_dir.join("EBWebView/Local State"), "x")?;

        assert!(
            !reset_if_runtime_changed(&paths, "host:154.0.0.1")?,
            "same runtime keeps it"
        );
        assert!(paths.webview2_dir.join("EBWebView/Local State").is_file());

        assert!(reset_if_runtime_changed(&paths, "bundled:153.0.1.1-x64")?);
        assert!(paths.webview2_dir.is_dir());
        assert_eq!(fs::read_dir(&paths.webview2_dir)?.count(), 0);
        assert_eq!(
            fs::read_to_string(paths.cache_dir.join(STAMP_FILE))?,
            "bundled:153.0.1.1-x64"
        );
        Ok(())
    }

    #[test]
    fn data_of_an_unknown_runtime_is_reset() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        fs::write(paths.webview2_dir.join("left-over"), "x")?;
        assert!(reset_if_runtime_changed(&paths, "host:154.0.0.1")?);
        assert!(!paths.webview2_dir.join("left-over").exists());
        Ok(())
    }

    #[test]
    fn reset_rules() {
        assert!(!needs_reset(None, "a", false));
        assert!(needs_reset(None, "a", true));
        assert!(!needs_reset(Some("a\n"), "a", true));
        assert!(needs_reset(Some("a"), "b", false));
    }
}

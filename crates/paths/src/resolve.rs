//! Per-app [`AppPaths`] inside a [`Layout`].

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::names::is_device_name;
use crate::{Environment, Layout, Mode, PathsError, detect_layout};

/// Name of the folder next to the per-app ones that holds suite-wide files, such as the
/// bundled `WebView2` runtime. Reserved: no app may be called this.
const SHARED_OTHER: &str = "shared";
/// File created and removed again to find out whether a folder can really be written.
const WRITE_PROBE: &str = ".write-probe";

/// Resolved folders for one app. Nothing is created until [`AppPaths::create_dirs`].
///
/// | Field | Location (inside `<root>/other/<app>/`) |
/// |---|---|
/// | `config_dir` | `configs/` (`settings.toml`, `keybindings.toml`) |
/// | `log_dir` | `logs/` |
/// | `cache_dir` | `cache/` (safe to delete) |
/// | `database_dir` | `database/` (durable app state, such as the recent list) |
/// | `webview2_dir` | `cache/webview2/` (the `WebView2` user data folder) |
/// | `temp_dir` | `cache/tmp/` (replaces `%TEMP%` for the launcher and its children) |
///
/// `documents/`, `licenses/` and `resources/` are reached through methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// The layout the paths were resolved in.
    pub layout: Layout,
    /// The app's name (lowercase kebab-case); also its folder name under `other/`.
    pub app: String,
    /// Config files (`settings.toml`, `keybindings.toml`).
    pub config_dir: PathBuf,
    /// Log files.
    pub log_dir: PathBuf,
    /// Disposable state.
    pub cache_dir: PathBuf,
    /// Durable app state.
    pub database_dir: PathBuf,
    /// The `WebView2` user data folder, identical for every window.
    pub webview2_dir: PathBuf,
    /// The folder `TEMP` and `TMP` are pointed at.
    pub temp_dir: PathBuf,
}

impl AppPaths {
    /// The mode the paths were resolved in.
    pub fn mode(&self) -> Mode {
        self.layout.mode
    }

    /// `other/<app>/`: everything this app owns.
    pub fn app_dir(&self) -> PathBuf {
        self.layout.other.join(&self.app)
    }

    /// `other/<app>/documents/`.
    pub fn documents_dir(&self) -> PathBuf {
        self.app_dir().join("documents")
    }

    /// `other/<app>/licenses/`.
    pub fn licenses_dir(&self) -> PathBuf {
        self.app_dir().join("licenses")
    }

    /// `other/<app>/resources/` (icons, utilities).
    pub fn resources_dir(&self) -> PathBuf {
        self.app_dir().join("resources")
    }

    /// `storage/vault/`: the encrypted files.
    pub fn vault_dir(&self) -> PathBuf {
        self.layout.storage.join("vault")
    }

    /// `cache/vault-session/`: where unlocked vault files are decrypted. Wiped at every start.
    pub fn vault_session_dir(&self) -> PathBuf {
        self.cache_dir.join("vault-session")
    }

    /// `storage/users/<profile>`; a name that is not one plain folder name gives
    /// `storage/users/_invalid`. See [`Layout::profile_dir`].
    pub fn profile_dir(&self, profile: &str) -> PathBuf {
        self.layout.profile_dir(profile)
    }

    /// `storage/users/<profile>`, or an error for a name that is not one plain folder name.
    ///
    /// # Errors
    ///
    /// [`PathsError::InvalidName`], see [`Layout::try_profile_dir`].
    pub fn try_profile_dir(&self, profile: &str) -> Result<PathBuf, PathsError> {
        self.layout.try_profile_dir(profile)
    }

    /// `other/shared/webview2/`: where the optional bundled `WebView2` runtime is unpacked, one
    /// `<version>-x64` folder per version.
    pub fn bundled_webview2_root(&self) -> PathBuf {
        self.layout.other.join(SHARED_OTHER).join("webview2")
    }

    /// Creates the app's folders and checks that they can be written.
    ///
    /// Creates `configs`, `cache`, `database`, `logs`, `documents`, `licenses`, `resources`,
    /// the `WebView2` data folder and the temp folder. It does not touch `storage/` (the user's
    /// files and the vault belong to their owners).
    ///
    /// # Errors
    ///
    /// [`PathsError::NotWritable`] if the install folder refuses writes, even when every folder
    /// already exists (a write-protected drive). There is no fallback to another location.
    pub fn create_dirs(&self) -> Result<(), PathsError> {
        self.create_dirs_using(|dir| fs::create_dir_all(dir), write_probe)
    }

    /// The whole of [`AppPaths::create_dirs`] with both file system steps replaced: `mkdir`
    /// creates each folder (and its missing parents) and `probe` proves that the cache folder
    /// can be written. Test hook, not a stable API.
    ///
    /// # Errors
    ///
    /// The first failure: [`PathsError::NotWritable`] for permission and read-only errors from
    /// `mkdir` and for any error from `probe`, [`PathsError::Io`] for other `mkdir` errors.
    #[doc(hidden)]
    pub fn create_dirs_using(
        &self,
        mkdir: impl FnMut(&Path) -> io::Result<()>,
        probe: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<(), PathsError> {
        self.create_dirs_with(mkdir)?;
        probe(&self.cache_dir).map_err(|_| PathsError::NotWritable(self.cache_dir.clone()))
    }

    /// Like [`AppPaths::create_dirs`] but creates each folder with `mkdir` and skips the
    /// check that the folders can be written. Test hook, not a stable API; `mkdir` must create
    /// missing parents.
    ///
    /// # Errors
    ///
    /// The first failure, as [`PathsError::NotWritable`] for permission and read-only errors and
    /// [`PathsError::Io`] for the rest.
    #[doc(hidden)]
    pub fn create_dirs_with(
        &self,
        mut mkdir: impl FnMut(&Path) -> io::Result<()>,
    ) -> Result<(), PathsError> {
        for dir in [
            &self.config_dir,
            &self.log_dir,
            &self.cache_dir,
            &self.database_dir,
            &self.webview2_dir,
            &self.temp_dir,
            &self.documents_dir(),
            &self.licenses_dir(),
            &self.resources_dir(),
        ] {
            mkdir(dir).map_err(|source| PathsError::from_io(dir, source))?;
        }
        Ok(())
    }
}

impl Layout {
    /// Paths for `app` (lowercase kebab-case) inside this layout.
    pub fn app(&self, app: &str) -> Result<AppPaths, PathsError> {
        validate(app)?;
        let app_dir = self.other.join(app);
        let cache_dir = app_dir.join("cache");
        Ok(AppPaths {
            config_dir: app_dir.join("configs"),
            log_dir: app_dir.join("logs"),
            database_dir: app_dir.join("database"),
            webview2_dir: cache_dir.join("webview2"),
            temp_dir: cache_dir.join("tmp"),
            cache_dir,
            app: app.to_owned(),
            layout: self.clone(),
        })
    }
}

/// Resolves paths for `app` from the real process environment and creates its folders.
///
/// If the install folder cannot be found or written this fails; it never falls back to the
/// PC's user folders.
pub fn resolve(app: &str) -> Result<AppPaths, PathsError> {
    let paths = resolve_with(app, &Environment::detect())?;
    paths.create_dirs()?;
    Ok(paths)
}

/// Resolves paths for `app` from an explicit [`Environment`] without writing to the disk
/// (beyond looking at the install folder's shape).
pub fn resolve_with(app: &str, env: &Environment) -> Result<AppPaths, PathsError> {
    validate(app)?;
    detect_layout(env)?.app(app)
}

/// Proves that `dir` can be written by creating a probe file in it.
///
/// A drive set to read-only lets `create_dir_all` succeed on folders that already exist, so
/// creating folders alone does not prove anything. Any error from creating the file means "not
/// writable" to the caller.
fn write_probe(dir: &Path) -> io::Result<()> {
    write_probe_with(dir, |probe| fs::remove_file(probe))
}

/// [`write_probe`] with the clean-up step replaced.
///
/// Removing the probe is best effort: on Windows, Defender or the search indexer may still hold
/// the new file open and make the delete fail with a sharing violation. That is not a reason to
/// refuse to start, and a stale empty probe file is harmless (the next run reuses it).
fn write_probe_with(dir: &Path, remove: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<()> {
    let probe = dir.join(WRITE_PROBE);
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&probe)?;
    let _ = remove(&probe);
    Ok(())
}

fn validate(app: &str) -> Result<(), PathsError> {
    let valid = !app.is_empty()
        && !app.starts_with('-')
        && !app.ends_with('-')
        && app != SHARED_OTHER
        && !is_device_name(app)
        && app
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if valid {
        Ok(())
    } else {
        Err(PathsError::InvalidName(app.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Environment;
    use genslate_testing::{TempTree, fake_repo, fake_suite};
    use std::error::Error;
    use std::io::{Error as IoError, ErrorKind};

    type TestResult = Result<(), Box<dyn Error>>;

    fn suite_paths(app: &str) -> Result<(TempTree, PathBuf, AppPaths), Box<dyn Error>> {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        let env = Environment {
            exe: Some(root.join("programs/genslate/launcher/slatecore-launcher.exe")),
            ..Environment::default()
        };
        let paths = resolve_with(app, &env)?;
        Ok((tmp, root, paths))
    }

    #[test]
    fn suite_paths_live_in_other_app() -> TestResult {
        let (_tmp, root, paths) = suite_paths("launcher")?;
        let app = root.join("other/launcher");
        assert_eq!(paths.mode(), Mode::Suite);
        assert_eq!(paths.app, "launcher");
        assert_eq!(paths.config_dir, app.join("configs"));
        assert_eq!(paths.log_dir, app.join("logs"));
        assert_eq!(paths.cache_dir, app.join("cache"));
        assert_eq!(paths.database_dir, app.join("database"));
        assert_eq!(paths.webview2_dir, app.join("cache/webview2"));
        assert_eq!(paths.temp_dir, app.join("cache/tmp"));
        assert_eq!(paths.app_dir(), app);
        assert_eq!(paths.documents_dir(), app.join("documents"));
        assert_eq!(paths.licenses_dir(), app.join("licenses"));
        assert_eq!(paths.resources_dir(), app.join("resources"));
        Ok(())
    }

    #[test]
    fn storage_and_vault_paths_follow_the_layout() -> TestResult {
        let (_tmp, root, paths) = suite_paths("launcher")?;
        assert_eq!(paths.vault_dir(), root.join("storage/vault"));
        assert_eq!(
            paths.vault_session_dir(),
            root.join("other/launcher/cache/vault-session")
        );
        assert_eq!(
            paths.profile_dir("shared"),
            root.join("storage/users/shared")
        );
        assert_eq!(
            paths.bundled_webview2_root(),
            root.join("other/shared/webview2")
        );
        Ok(())
    }

    #[test]
    fn dev_paths_use_the_launcher_other_folder() -> TestResult {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let env = Environment {
            debug: true,
            repo_root: Some(repo.clone()),
            exe: Some(repo.join("target/debug/slatecore-launcher.exe")),
            ..Environment::default()
        };
        let paths = resolve_with("launcher", &env)?;
        let launcher = repo.join("programs/desktop/launcher");
        assert_eq!(paths.mode(), Mode::Dev);
        assert_eq!(paths.config_dir, launcher.join("other/launcher/configs"));
        assert_eq!(paths.log_dir, launcher.join("other/launcher/logs"));
        assert_eq!(paths.vault_dir(), launcher.join("installDir/storage/vault"));
        assert_eq!(
            paths.bundled_webview2_root(),
            launcher.join("other/shared/webview2")
        );
        Ok(())
    }

    #[test]
    fn other_apps_get_their_own_folder_under_other() -> TestResult {
        let (_tmp, root, paths) = suite_paths("explorer")?;
        assert_eq!(paths.config_dir, root.join("other/explorer/configs"));
        Ok(())
    }

    #[test]
    fn invalid_names_are_rejected() {
        let env = Environment::default();
        for bad in [
            "",
            "Launcher",
            "../x",
            "-x",
            "a b",
            "a/b",
            r"a\b",
            "a.b",
            "\u{fc}n\u{ef}",
            "shared",
            // Trailing '-', '.' and space (Windows drops the last two silently).
            "launcher-",
            "launcher.",
            "launcher ",
            "-",
            // Windows device names.
            "con",
            "prn",
            "aux",
            "nul",
            "com1",
            "com9",
            "lpt1",
            "lpt9",
        ] {
            assert!(
                matches!(resolve_with(bad, &env), Err(PathsError::InvalidName(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn ordinary_kebab_case_names_are_accepted() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        for good in [
            "launcher", "explorer", "my-app", "app2", "a", "a-b-c", "com", "com0", "lpt10",
            "console",
        ] {
            assert!(paths.layout.app(good).is_ok(), "{good:?}");
        }
        Ok(())
    }

    #[test]
    fn invalid_names_are_reported_before_the_install_is_looked_up() {
        // The environment knows nothing, yet the name is what gets reported.
        let error = resolve_with("Bad Name", &Environment::default());
        assert!(matches!(error, Err(PathsError::InvalidName(name)) if name == "Bad Name"));
    }

    #[test]
    fn create_dirs_makes_every_app_folder() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        paths.create_dirs()?;
        for dir in [
            &paths.config_dir,
            &paths.log_dir,
            &paths.cache_dir,
            &paths.database_dir,
            &paths.webview2_dir,
            &paths.temp_dir,
        ] {
            assert!(dir.is_dir(), "{}", dir.display());
        }
        for dir in [
            paths.documents_dir(),
            paths.licenses_dir(),
            paths.resources_dir(),
        ] {
            assert!(dir.is_dir(), "{}", dir.display());
        }
        Ok(())
    }

    #[test]
    fn create_dirs_is_repeatable_and_leaves_no_probe_behind() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        paths.create_dirs()?;
        paths.create_dirs()?;
        let mut names: Vec<_> = fs::read_dir(&paths.cache_dir)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<_, _>>()?;
        names.sort();
        assert_eq!(names, ["tmp", "webview2"]);
        Ok(())
    }

    #[test]
    fn create_dirs_does_not_touch_user_data_or_the_vault() -> TestResult {
        let (_tmp, root, paths) = suite_paths("launcher")?;
        paths.create_dirs()?;
        assert_eq!(fs::read_dir(root.join("storage/vault"))?.count(), 0);
        assert!(!paths.vault_session_dir().exists());
        assert!(!paths.bundled_webview2_root().exists());
        Ok(())
    }

    #[test]
    fn permission_errors_become_not_writable_with_the_folder() -> TestResult {
        let (_tmp, root, paths) = suite_paths("launcher")?;
        for kind in [ErrorKind::PermissionDenied, ErrorKind::ReadOnlyFilesystem] {
            let result = paths.create_dirs_with(|_| Err(IoError::from(kind)));
            match result {
                Err(PathsError::NotWritable(path)) => assert!(path.starts_with(&root)),
                other => panic!("{kind:?}: expected NotWritable, got {other:?}"),
            }
        }
        Ok(())
    }

    #[test]
    fn other_errors_stay_io_errors() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        let result = paths.create_dirs_with(|_| Err(IoError::from(ErrorKind::StorageFull)));
        assert!(matches!(result, Err(PathsError::Io { .. })), "{result:?}");
        Ok(())
    }

    #[test]
    fn creation_stops_at_the_first_failure() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        let mut calls = 0;
        let result = paths.create_dirs_with(|_| {
            calls += 1;
            Err(IoError::from(ErrorKind::PermissionDenied))
        });
        assert!(result.is_err());
        assert_eq!(calls, 1);
        Ok(())
    }

    #[test]
    fn probe_failure_of_any_kind_means_not_writable() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        for kind in [
            ErrorKind::PermissionDenied,
            ErrorKind::ReadOnlyFilesystem,
            ErrorKind::NotFound,
            ErrorKind::StorageFull,
        ] {
            let result = paths
                .create_dirs_using(|dir| fs::create_dir_all(dir), |_| Err(IoError::from(kind)));
            assert!(
                matches!(&result, Err(PathsError::NotWritable(path)) if *path == paths.cache_dir),
                "{kind:?}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn the_probe_runs_in_the_cache_folder_after_every_folder_exists() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        let mut probed = Vec::new();
        paths.create_dirs_using(
            |dir| fs::create_dir_all(dir),
            |dir| {
                probed.push(dir.to_path_buf());
                assert!(paths.webview2_dir.is_dir());
                assert!(paths.resources_dir().is_dir());
                Ok(())
            },
        )?;
        assert_eq!(probed.as_slice(), std::slice::from_ref(&paths.cache_dir));
        Ok(())
    }

    #[test]
    fn the_probe_is_not_reached_when_a_folder_cannot_be_created() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        let mut probed = false;
        let result = paths.create_dirs_using(
            |_| Err(IoError::from(ErrorKind::PermissionDenied)),
            |_| {
                probed = true;
                Ok(())
            },
        );
        assert!(matches!(result, Err(PathsError::NotWritable(_))));
        assert!(!probed);
        Ok(())
    }

    #[test]
    fn create_dirs_with_does_not_probe() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        paths.create_dirs_with(|dir| fs::create_dir_all(dir))?;
        assert_eq!(fs::read_dir(&paths.cache_dir)?.count(), 2);
        Ok(())
    }

    #[test]
    fn writing_the_probe_to_something_that_is_not_a_folder_fails() -> TestResult {
        let tree = TempTree::new()?.file("a-file", "x")?;
        assert!(write_probe(&tree.join("a-file")).is_err());
        Ok(())
    }

    #[test]
    fn the_probe_succeeds_in_a_writable_folder() -> TestResult {
        let tree = TempTree::new()?;
        write_probe(tree.path())?;
        assert_eq!(fs::read_dir(tree.path())?.count(), 0);
        Ok(())
    }

    #[test]
    fn failing_to_remove_the_probe_is_not_an_error() -> TestResult {
        // Defender or the search indexer can hold the new file open for a moment on Windows.
        let tree = TempTree::new()?;
        for kind in [ErrorKind::PermissionDenied, ErrorKind::Other] {
            write_probe_with(tree.path(), |_| Err(IoError::from(kind)))?;
        }
        Ok(())
    }

    #[test]
    fn a_stale_empty_probe_is_harmless() -> TestResult {
        let (_tmp, _root, paths) = suite_paths("launcher")?;
        fs::create_dir_all(&paths.cache_dir)?;
        fs::write(paths.cache_dir.join(WRITE_PROBE), "")?;
        paths.create_dirs()?;
        paths.create_dirs()?;
        Ok(())
    }
}

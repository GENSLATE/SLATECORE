//! The portable layout as seen through the public API only.

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use genslate_paths::{AppPaths, Environment, Mode, PathsError, detect_layout, resolve_with};
use genslate_testing::{NoTraceGuard, TempTree, fake_repo, fake_suite};

type TestResult = Result<(), Box<dyn Error>>;

fn suite_env(root: &Path) -> Environment {
    Environment {
        exe: Some(root.join("programs/genslate/launcher/slatecore-launcher.exe")),
        ..Environment::default()
    }
}

fn dev_env(repo: &Path) -> Environment {
    Environment {
        debug: true,
        repo_root: Some(repo.to_path_buf()),
        exe: Some(repo.join("target/debug/slatecore-launcher.exe")),
        ..Environment::default()
    }
}

/// Every folder an app can be handed, as the launcher would use them.
fn every_path(paths: &AppPaths) -> Vec<PathBuf> {
    let mut all = vec![
        paths.layout.root.clone(),
        paths.layout.other.clone(),
        paths.layout.storage.clone(),
        paths.config_dir.clone(),
        paths.log_dir.clone(),
        paths.cache_dir.clone(),
        paths.database_dir.clone(),
        paths.webview2_dir.clone(),
        paths.temp_dir.clone(),
        paths.app_dir(),
        paths.documents_dir(),
        paths.licenses_dir(),
        paths.resources_dir(),
        paths.vault_dir(),
        paths.vault_session_dir(),
        paths.profile_dir("shared"),
        paths.bundled_webview2_root(),
    ];
    all.extend(paths.layout.programs.clone());
    all
}

fn relative_to_root(paths: &AppPaths) -> Result<Vec<PathBuf>, std::path::StripPrefixError> {
    every_path(paths)
        .iter()
        .map(|path| path.strip_prefix(&paths.layout.root).map(Path::to_path_buf))
        .collect()
}

#[test]
fn suite_mode_detected_from_exe_shape() -> TestResult {
    let tmp = TempTree::new()?;
    let root = fake_suite(tmp.path())?;
    let layout = detect_layout(&suite_env(&root))?;
    assert_eq!(layout.mode, Mode::Suite);
    assert_eq!(layout.root, root);
    assert_eq!(layout.other, root.join("other"));
    assert_eq!(layout.programs, Some(root.join("programs")));
    assert_eq!(layout.storage, root.join("storage"));

    // A debug build staged into an install folder behaves like the real thing.
    let staged = Environment {
        debug: true,
        repo_root: Some(PathBuf::from("/some/repo")),
        ..suite_env(&root)
    };
    assert_eq!(detect_layout(&staged)?.mode, Mode::Suite);
    Ok(())
}

#[test]
fn dev_mode_uses_repo_mockup_and_launcher_other() -> TestResult {
    let tmp = TempTree::new()?;
    let repo = fake_repo(tmp.path())?;
    let launcher = repo.join("programs/desktop/launcher");

    let paths = resolve_with("launcher", &dev_env(&repo))?;
    assert_eq!(paths.mode(), Mode::Dev);
    assert_eq!(paths.layout.root, repo);
    assert_eq!(paths.layout.other, launcher.join("other"));
    assert_eq!(
        paths.layout.programs,
        Some(launcher.join("installDir/programs"))
    );
    assert_eq!(paths.layout.storage, launcher.join("installDir/storage"));
    assert_eq!(paths.config_dir, launcher.join("other/launcher/configs"));
    assert_eq!(paths.vault_dir(), launcher.join("installDir/storage/vault"));

    // Only debug builds look at the checkout; a release build has no install to use.
    let release = Environment {
        debug: false,
        ..dev_env(&repo)
    };
    assert!(matches!(
        resolve_with("launcher", &release),
        Err(PathsError::NotInstalled)
    ));
    Ok(())
}

/// The folders and files the create pipeline asks the OS to touch.
type Touched = RefCell<Vec<PathBuf>>;

/// A medium that refuses every write inside `root` with `kind` but would happily accept any
/// other place. An implementation that fell back to another location would therefore succeed
/// (or be seen touching a path outside `root`) instead of failing.
fn refuse_inside<'a>(
    root: &'a Path,
    kind: io::ErrorKind,
    touched: &'a Touched,
) -> impl FnMut(&Path) -> io::Result<()> + 'a {
    move |dir| {
        touched.borrow_mut().push(dir.to_path_buf());
        if dir.starts_with(root) {
            Err(io::Error::from(kind))
        } else {
            Ok(())
        }
    }
}

#[test]
fn unwritable_install_is_not_writable_error_and_never_falls_back() -> TestResult {
    // Stand-ins for %TEMP%, %APPDATA% and %LOCALAPPDATA%: they must stay empty.
    let tmp = TempTree::new()?
        .dir("TEMP")?
        .dir("APPDATA")?
        .dir("LOCALAPPDATA")?;
    let os_dirs = [
        tmp.join("TEMP"),
        tmp.join("APPDATA"),
        tmp.join("LOCALAPPDATA"),
    ];
    let guard = NoTraceGuard::watching(os_dirs.clone())?;
    let suite = fake_suite(tmp.join("usb"))?;
    let repo = fake_repo(tmp.join("checkout"))?;
    let installs = [
        (suite.clone(), suite_env(&suite)),
        (repo.clone(), dev_env(&repo)),
    ];

    for (root, env) in installs {
        let paths = resolve_with("launcher", &env)?;
        let kinds = [
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::ReadOnlyFilesystem,
        ];
        for kind in kinds {
            // 1. Creating the folders is refused.
            let touched = Touched::default();
            let result = paths.create_dirs_using(
                refuse_inside(&root, kind, &touched),
                refuse_inside(&root, kind, &touched),
            );
            match result {
                Err(PathsError::NotWritable(path)) => {
                    assert!(path.starts_with(&root), "{kind:?}: {}", path.display());
                }
                other => panic!("{kind:?}: expected NotWritable, got {other:?}"),
            }
            assert_nothing_outside(&root, &os_dirs, &touched.borrow());
            // It stops at the first refusal instead of trying elsewhere.
            assert_eq!(touched.borrow().len(), 1, "{:?}", touched.borrow());

            // 2. The folders exist (read-only drive) but the write test is refused.
            let touched = Touched::default();
            let result = paths.create_dirs_using(
                |dir| {
                    touched.borrow_mut().push(dir.to_path_buf());
                    fs::create_dir_all(dir)
                },
                refuse_inside(&root, kind, &touched),
            );
            match result {
                Err(PathsError::NotWritable(path)) => assert_eq!(path, paths.cache_dir),
                other => panic!("{kind:?}: expected NotWritable, got {other:?}"),
            }
            assert_nothing_outside(&root, &os_dirs, &touched.borrow());
            assert_eq!(touched.borrow().last(), Some(&paths.cache_dir));
        }

        // Resolving again after the failures still answers with the install folder only.
        let again = resolve_with("launcher", &env)?;
        assert_eq!(again.mode(), paths.mode());
        assert!(
            every_path(&again)
                .iter()
                .all(|path| path.starts_with(&root))
        );
    }
    guard.assert_unchanged();
    Ok(())
}

/// Every path the pipeline touched is inside `root`, and none is under an OS folder.
fn assert_nothing_outside(root: &Path, os_dirs: &[PathBuf], touched: &[PathBuf]) {
    let appdata = env::var_os("APPDATA").map(PathBuf::from);
    for path in touched {
        assert!(path.starts_with(root), "touched {}", path.display());
        assert!(os_dirs.iter().all(|os| !path.starts_with(os)));
        assert!(appdata.as_ref().is_none_or(|dir| !path.starts_with(dir)));
    }
}

/// Puts a folder's permissions back when dropped, even if the test panics.
#[cfg(unix)]
struct RestorePermissions {
    path: PathBuf,
    mode: u32,
}

#[cfg(unix)]
impl Drop for RestorePermissions {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        // Best effort: the temporary folder is deleted right after.
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(self.mode));
    }
}

/// The same refusal with real file permissions (skipped where permissions are not enforced).
#[cfg(unix)]
#[test]
fn unwritable_folder_with_real_permissions_is_refused() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempTree::new()?;
    let root = fake_suite(tmp.path())?;
    let locked = root.join("other");
    let _restore = RestorePermissions {
        path: locked.clone(),
        mode: 0o755,
    };
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555))?;

    // Root (as in many CI containers) ignores permission bits: nothing to prove then.
    if fs::create_dir(locked.join(".privileged")).is_ok() {
        return Ok(());
    }
    let paths = resolve_with("launcher", &suite_env(&root))?;
    match paths.create_dirs() {
        Err(PathsError::NotWritable(path)) => assert!(path.starts_with(&root)),
        other => panic!("expected NotWritable, got {other:?}"),
    }
    Ok(())
}

#[test]
fn drive_letter_change_keeps_layout_relative() -> TestResult {
    // Two different roots stand in for `S:\` on one PC and `E:\` on another.
    let first = TempTree::new()?;
    let second = TempTree::new()?;
    let root_a = fake_suite(first.path())?;
    let root_b = fake_suite(second.join("some/other/place"))?;
    assert_ne!(root_a, root_b);

    let a = resolve_with("launcher", &suite_env(&root_a))?;
    let b = resolve_with("launcher", &suite_env(&root_b))?;
    assert_eq!(relative_to_root(&a)?, relative_to_root(&b)?);

    // Moving the whole tree (the drive got another letter) changes nothing relative to root.
    let moved = second.join("moved");
    fs::rename(&root_b, &moved)?;
    let c = resolve_with("launcher", &suite_env(&moved))?;
    assert_eq!(c.layout.root, moved);
    assert_eq!(relative_to_root(&a)?, relative_to_root(&c)?);

    // The same holds for the development checkout.
    let repo_a = fake_repo(first.path())?;
    let repo_b = fake_repo(second.join("elsewhere"))?;
    let dev_a = resolve_with("launcher", &dev_env(&repo_a))?;
    let dev_b = resolve_with("launcher", &dev_env(&repo_b))?;
    assert_eq!(relative_to_root(&dev_a)?, relative_to_root(&dev_b)?);
    Ok(())
}

#[test]
fn paths_with_spaces_and_unicode_resolve() -> TestResult {
    let outer = tempfile::Builder::new()
        .prefix("My Drive \u{fc} \u{2713} \u{65e5}\u{672c}\u{8a9e} ")
        .tempdir()?;
    let parent = outer.path().join("SLATECORE (copy 2) \u{2014} caf\u{e9}");
    let root = fake_suite(&parent)?;
    let paths = resolve_with("launcher", &suite_env(&root))?;
    paths.create_dirs()?;

    assert_eq!(paths.layout.root, root);
    assert!(paths.config_dir.is_dir());
    assert!(paths.webview2_dir.is_dir());
    assert!(paths.temp_dir.is_dir());
    for path in every_path(&paths) {
        assert!(path.starts_with(&root), "{}", path.display());
        // Persisted text and the webview data folder arguments need valid UTF-8.
        assert!(path.to_str().is_some(), "{}", path.display());
    }
    Ok(())
}

#[test]
fn unc_root_is_rejected_with_unc_path_error() {
    let unc = PathBuf::from(r"\\fileserver\share\SLATECORE");
    let inputs = [
        // Forced install folder.
        Environment {
            install_dir_override: Some(unc.clone()),
            ..Environment::default()
        },
        // Executable on a share.
        Environment {
            exe: Some(unc.join(r"programs\genslate\launcher\slatecore-launcher.exe")),
            ..Environment::default()
        },
        // Checkout on a share.
        Environment {
            debug: true,
            repo_root: Some(unc.clone()),
            ..Environment::default()
        },
        // The verbatim spelling of a share, and forward slashes.
        Environment {
            install_dir_override: Some(PathBuf::from(r"\\?\UNC\fileserver\share\SLATECORE")),
            ..Environment::default()
        },
        Environment {
            install_dir_override: Some(PathBuf::from("//fileserver/share/SLATECORE")),
            ..Environment::default()
        },
    ];
    for env in inputs {
        assert!(
            matches!(detect_layout(&env), Err(PathsError::UncPath(_))),
            "{env:?}"
        );
        assert!(
            matches!(resolve_with("launcher", &env), Err(PathsError::UncPath(_))),
            "{env:?}"
        );
    }
}

#[test]
fn every_app_path_is_inside_root() -> TestResult {
    let tmp = TempTree::new()?;
    let suite = fake_suite(tmp.path())?;
    let repo = fake_repo(tmp.path())?;
    let cases = [
        resolve_with("launcher", &suite_env(&suite))?,
        resolve_with("explorer", &suite_env(&suite))?,
        resolve_with("launcher", &dev_env(&repo))?,
    ];
    for paths in cases {
        for path in every_path(&paths) {
            assert!(
                path.starts_with(&paths.layout.root),
                "{:?}: {} is outside {}",
                paths.mode(),
                path.display(),
                paths.layout.root.display()
            );
            assert!(
                !path.components().any(|part| part.as_os_str() == ".."),
                "{}",
                path.display()
            );
        }
    }
    Ok(())
}

#[test]
fn creating_folders_writes_only_inside_the_install() -> TestResult {
    let tmp = TempTree::new()?
        .dir("TEMP")?
        .dir("APPDATA")?
        .dir("LOCALAPPDATA")?;
    let os_guard = NoTraceGuard::watching([
        tmp.join("TEMP"),
        tmp.join("APPDATA"),
        tmp.join("LOCALAPPDATA"),
    ])?;
    let root = fake_suite(tmp.join("usb"))?;
    let user_data_guard =
        NoTraceGuard::watching([root.join("programs"), root.join("storage")])?.depth(4)?;

    let paths = resolve_with("launcher", &suite_env(&root))?;
    paths.create_dirs()?;

    assert!(paths.webview2_dir.is_dir());
    os_guard.assert_unchanged();
    user_data_guard.assert_unchanged();
    Ok(())
}

#[test]
fn a_folder_without_the_install_shape_is_not_installed() -> TestResult {
    let tmp = TempTree::new()?.dir("loose/programs/genslate/launcher")?;
    let env = Environment {
        exe: Some(tmp.join("loose/programs/genslate/launcher/slatecore-launcher.exe")),
        ..Environment::default()
    };
    assert!(matches!(
        resolve_with("launcher", &env),
        Err(PathsError::NotInstalled)
    ));
    Ok(())
}

#[test]
fn profile_dir_never_leaves_storage_users() -> TestResult {
    let tmp = TempTree::new()?;
    let root = fake_suite(tmp.path())?;
    let paths = resolve_with("launcher", &suite_env(&root))?;
    let users = root.join("storage/users");
    for hostile in ["../x", "/etc", r"C:\x", "", "..", "a/b", "con"] {
        let dir = paths.profile_dir(hostile);
        assert!(dir.starts_with(&users), "{hostile:?} -> {}", dir.display());
        assert_eq!(dir, users.join("_invalid"), "{hostile:?}");
        assert!(
            paths.layout.try_profile_dir(hostile).is_err(),
            "{hostile:?}"
        );
    }
    assert_eq!(paths.profile_dir("alice"), users.join("alice"));
    Ok(())
}

#[test]
fn install_dir_override_needs_the_suite_shape() -> TestResult {
    let tmp = TempTree::new()?.dir("empty")?;
    let root = fake_suite(tmp.path())?;
    let forced = |dir: PathBuf| Environment {
        install_dir_override: Some(dir),
        ..Environment::default()
    };
    assert_eq!(detect_layout(&forced(root.clone()))?.root, root);
    assert!(matches!(
        detect_layout(&forced(tmp.join("empty"))),
        Err(PathsError::NotInstalled)
    ));
    Ok(())
}

#[test]
fn programs_folder_keeps_its_real_name() -> TestResult {
    let tree = TempTree::new()?
        .dir("usb/Programs/genslate/launcher")?
        .dir("usb/other")?
        .dir("usb/storage")?;
    let env = Environment {
        exe: Some(tree.join("usb/Programs/genslate/launcher/slatecore-launcher.exe")),
        ..Environment::default()
    };
    let layout = detect_layout(&env)?;
    assert_eq!(layout.programs, Some(tree.join("usb/Programs")));
    Ok(())
}

/// Compiles only while `PathsError` can be matched without a wildcard arm.
#[test]
fn errors_can_be_matched_exhaustively() {
    let describe = |error: &PathsError| match error {
        PathsError::NotInstalled => "not installed",
        PathsError::NotWritable(_) => "not writable",
        PathsError::UncPath(_) => "unc",
        PathsError::InvalidName(_) => "invalid name",
        PathsError::Io { .. } => "io",
    };
    assert_eq!(describe(&PathsError::NotInstalled), "not installed");
}

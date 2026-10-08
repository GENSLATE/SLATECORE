//! Which folder plays the role of `installDir` for the running launcher.
//!
//! Every mode has the same shape so the rest of the code never cares where it runs:
//!
//! ```text
//! <root>/
//! ├─ programs/   genslate/<app>/, portableapps.com/, portapps.io/
//! ├─ other/      <app>/{configs,cache,database,logs,documents,licenses,resources}, shared/
//! └─ storage/    users/<profile>/{Desktop,Documents,…}, vault/
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use crate::names::is_plain_name;
use crate::unc::is_unc;
use crate::{Environment, PathsError};

/// Where the launcher keeps its files. See [`detect_layout`] for the precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Debug build inside the repo: `programs/` and `storage/` of the mock-up install folder
    /// `programs/desktop/launcher/installDir/`, and `programs/desktop/launcher/other/` for
    /// `other/`.
    Dev,
    /// Inside an install folder: `<installDir>/programs/…/<exe>` next to `other/` and
    /// `storage/`.
    Suite,
}

/// The resolved folders of a [`Mode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// How the layout was found.
    pub mode: Mode,
    /// `installDir` (suite) or the repository (dev). Every other folder is below it.
    pub root: PathBuf,
    /// `other/`: configs, cache, logs, database and documents, one folder per app.
    pub other: PathBuf,
    /// `programs/`: the apps the launcher lists.
    pub programs: Option<PathBuf>,
    /// `storage/`: the user's portable files and the vault.
    pub storage: PathBuf,
}

/// The profile every install has; more profiles may be added later.
pub const SHARED_PROFILE: &str = "shared";
/// The launcher's mock-up inside a checkout, in dev mode.
const DEV_LAUNCHER_DIR: [&str; 3] = ["programs", "desktop", "launcher"];
/// Folder name [`Layout::profile_dir`] answers with for a profile name that is not usable.
const INVALID_PROFILE: &str = "_invalid";
/// The mock-up install folder inside [`DEV_LAUNCHER_DIR`]; it supplies `programs/` and `storage/`.
const DEV_INSTALL_DIR: &str = "installDir";

impl Layout {
    /// `programs` is the folder as it exists on disk (`Programs` stays `Programs`).
    fn suite(root: PathBuf, programs: PathBuf) -> Self {
        Self {
            mode: Mode::Suite,
            other: root.join("other"),
            programs: Some(programs),
            storage: root.join("storage"),
            root,
        }
    }

    fn dev(repo: PathBuf) -> Self {
        let launcher = DEV_LAUNCHER_DIR
            .iter()
            .fold(repo.clone(), |dir, part| dir.join(part));
        let install = launcher.join(DEV_INSTALL_DIR);
        Self {
            mode: Mode::Dev,
            other: launcher.join("other"),
            programs: Some(install.join("programs")),
            storage: install.join("storage"),
            root: repo,
        }
    }

    /// `storage/users/<profile>`, or `storage/users/_invalid` when `profile` is not one plain
    /// folder name (empty, `..`, `/etc`, `C:\x`, `a/b`, a device name, …), so the result can
    /// never leave `storage/users`. Use [`Layout::try_profile_dir`] to learn about the mistake.
    pub fn profile_dir(&self, profile: &str) -> PathBuf {
        self.try_profile_dir(profile)
            .unwrap_or_else(|_| self.users_dir().join(INVALID_PROFILE))
    }

    /// `storage/users/<profile>`.
    ///
    /// # Errors
    ///
    /// [`PathsError::InvalidName`] unless `profile` is one plain folder name: not empty, not `.`
    /// or `..`, without a separator, drive colon, wildcard, control character, trailing dot or
    /// space, and not a Windows device name such as `con`.
    pub fn try_profile_dir(&self, profile: &str) -> Result<PathBuf, PathsError> {
        if is_plain_name(profile) {
            Ok(self.users_dir().join(profile))
        } else {
            Err(PathsError::InvalidName(profile.to_owned()))
        }
    }

    fn users_dir(&self) -> PathBuf {
        self.storage.join("users")
    }

    /// The display name of the install: the root folder's name (`SLATECORE`, `MY-USB`, …).
    pub fn name(&self) -> Option<&str> {
        self.root.file_name().and_then(|name| name.to_str())
    }
}

/// Detects the layout from `env`.
///
/// Precedence: [`crate::INSTALL_DIR_ENV`] override, then suite (the exe's folder shape), then dev
/// (debug build in the repo). Suite beats dev so a debug build staged into an install folder
/// behaves like the real thing. Nothing else is tried: there is no standalone mode and no OS
/// fallback.
///
/// # Errors
///
/// [`PathsError::UncPath`] if the override, the exe or the repo is on a network share, checked
/// before anything else; [`PathsError::NotInstalled`] if no install folder is found.
pub fn detect_layout(env: &Environment) -> Result<Layout, PathsError> {
    let install_dir_override = env.install_dir_override.as_deref().map(clean);
    let exe = env.exe.as_deref().map(clean);
    let repo_root = env.repo_root.as_deref().map(clean);
    for path in [&install_dir_override, &exe, &repo_root]
        .into_iter()
        .flatten()
    {
        if is_unc(path) {
            return Err(PathsError::UncPath(path.clone()));
        }
    }

    if let Some(root) = install_dir_override {
        // A forced install must still look like one: no shape, no install (and no folders are
        // created in a mistyped place).
        return if has_install_shape(&root) {
            let programs = programs_dir(&root);
            Ok(Layout::suite(root, programs))
        } else {
            Err(PathsError::NotInstalled)
        };
    }
    if let Some((root, programs)) = exe.as_deref().and_then(find_install) {
        return Ok(Layout::suite(root, programs));
    }
    if env.debug
        && let Some(repo) = repo_root
    {
        return Ok(Layout::dev(repo));
    }
    Err(PathsError::NotInstalled)
}

/// `Some(X)` when `exe` lives below `X/programs/` and `X` also has `other/` and `storage/`.
///
/// The nearest such `X` wins. The `programs` folder name is matched case-insensitively (Windows
/// file systems are).
pub fn find_install_dir(exe: &Path) -> Option<PathBuf> {
    find_install(exe).map(|(root, _programs)| root)
}

/// Like [`find_install_dir`], also returning the `programs` folder with the name it really has.
fn find_install(exe: &Path) -> Option<(PathBuf, PathBuf)> {
    exe.ancestors().skip(1).find_map(|dir| {
        let named_programs = dir
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("programs"));
        let root = dir.parent().filter(|root| !root.as_os_str().is_empty())?;
        (named_programs && has_install_shape(root)).then(|| (root.to_path_buf(), dir.to_path_buf()))
    })
}

/// `root` has the `other/` and `storage/` folders of an install.
fn has_install_shape(root: &Path) -> bool {
    root.join("other").is_dir() && root.join("storage").is_dir()
}

/// The `programs` folder of `root` as it exists (any letter case), else `root/programs`.
fn programs_dir(root: &Path) -> PathBuf {
    let exact = root.join("programs");
    if exact.is_dir() {
        return exact;
    }
    fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("programs"))
                && entry.path().is_dir()
        })
        .map_or(exact, |entry| entry.path())
}

/// Drops a `\\?\` prefix that Windows adds when a path is canonicalised.
fn clean(path: &Path) -> PathBuf {
    dunce::simplified(path).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_testing::{TempTree, fake_repo, fake_suite};
    use std::error::Error;

    type TestResult = Result<(), Box<dyn Error>>;

    fn env_for_exe(exe: PathBuf) -> Environment {
        Environment {
            exe: Some(exe),
            ..Environment::default()
        }
    }

    fn launcher_exe(root: &Path) -> PathBuf {
        root.join("programs/genslate/launcher/slatecore-launcher.exe")
    }

    #[test]
    fn suite_is_detected_from_the_folder_shape() -> TestResult {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        let layout = detect_layout(&env_for_exe(launcher_exe(&root)))?;
        assert_eq!(layout.mode, Mode::Suite);
        assert_eq!(layout.root, root);
        assert_eq!(layout.other, root.join("other"));
        assert_eq!(layout.programs, Some(root.join("programs")));
        assert_eq!(layout.storage, root.join("storage"));
        assert_eq!(
            layout.profile_dir(SHARED_PROFILE),
            root.join("storage/users/shared")
        );
        Ok(())
    }

    #[test]
    fn install_dir_can_have_any_name() -> TestResult {
        let tree = TempTree::new()?
            .dir("My Portable Stuff/other")?
            .dir("My Portable Stuff/storage")?
            .dir("My Portable Stuff/programs/GenSlate/launcher")?;
        let exe = tree.join("My Portable Stuff/programs/GenSlate/launcher/launcher.exe");
        let layout = detect_layout(&env_for_exe(exe))?;
        assert_eq!(layout.mode, Mode::Suite);
        assert_eq!(layout.name(), Some("My Portable Stuff"));
        Ok(())
    }

    #[test]
    fn any_app_below_programs_finds_the_install_dir() -> TestResult {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        for exe in [
            "programs/portapps.io/some-app/some-app.exe",
            "programs/portableapps.com/Tool/App/tool.exe",
            "programs/genslate/launcher/bin/deeper/launcher.exe",
        ] {
            assert_eq!(
                find_install_dir(&root.join(exe)),
                Some(root.clone()),
                "{exe}"
            );
        }
        Ok(())
    }

    #[test]
    fn programs_folder_name_is_matched_case_insensitively() -> TestResult {
        let tree = TempTree::new()?
            .dir("x/Programs/genslate/launcher")?
            .dir("x/other")?
            .dir("x/storage")?;
        let exe = tree.join("x/Programs/genslate/launcher/launcher.exe");
        assert_eq!(find_install_dir(&exe), Some(tree.join("x")));
        Ok(())
    }

    #[test]
    fn nearest_install_dir_wins() -> TestResult {
        let tmp = TempTree::new()?;
        let outer = fake_suite(tmp.path())?;
        let inner = fake_suite(outer.join("programs/genslate/nested"))?;
        assert_eq!(find_install_dir(&launcher_exe(&inner)), Some(inner));
        Ok(())
    }

    #[test]
    fn a_missing_part_of_the_shape_is_not_a_suite() -> TestResult {
        for missing in ["other", "storage"] {
            let tmp = TempTree::new()?;
            let root = fake_suite(tmp.path())?;
            fs::remove_dir_all(root.join(missing))?;
            let result = detect_layout(&env_for_exe(launcher_exe(&root)));
            assert!(
                matches!(result, Err(PathsError::NotInstalled)),
                "without {missing}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn an_exe_outside_programs_is_not_a_suite() -> TestResult {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        let result = detect_layout(&env_for_exe(root.join("launcher.exe")));
        assert!(
            matches!(result, Err(PathsError::NotInstalled)),
            "{result:?}"
        );
        Ok(())
    }

    #[test]
    fn suite_wins_over_dev() -> TestResult {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        let env = Environment {
            debug: true,
            repo_root: Some(PathBuf::from("/repo")),
            ..env_for_exe(launcher_exe(&root))
        };
        assert_eq!(detect_layout(&env)?.mode, Mode::Suite);
        Ok(())
    }

    #[test]
    fn dev_uses_the_repo_mockup_and_the_launcher_other_folder() -> TestResult {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let env = Environment {
            debug: true,
            repo_root: Some(repo.clone()),
            ..env_for_exe(repo.join("target/debug/slatecore-launcher.exe"))
        };
        let layout = detect_layout(&env)?;
        let launcher = repo.join("programs/desktop/launcher");
        assert_eq!(layout.mode, Mode::Dev);
        assert_eq!(layout.root, repo);
        assert_eq!(layout.other, launcher.join("other"));
        assert_eq!(layout.programs, Some(launcher.join("installDir/programs")));
        assert_eq!(layout.storage, launcher.join("installDir/storage"));
        Ok(())
    }

    #[test]
    fn release_builds_never_use_the_repo() -> TestResult {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let env = Environment {
            debug: false,
            repo_root: Some(repo.clone()),
            ..env_for_exe(repo.join("target/release/slatecore-launcher.exe"))
        };
        assert!(matches!(detect_layout(&env), Err(PathsError::NotInstalled)));
        Ok(())
    }

    #[test]
    fn a_debug_build_outside_a_checkout_is_not_installed() {
        let env = Environment {
            debug: true,
            repo_root: None,
            ..env_for_exe(PathBuf::from("/somewhere/else/app.exe"))
        };
        assert!(matches!(detect_layout(&env), Err(PathsError::NotInstalled)));
    }

    #[test]
    fn override_forces_a_suite() -> TestResult {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        let env = Environment {
            install_dir_override: Some(root.clone()),
            ..Environment::default()
        };
        let layout = detect_layout(&env)?;
        assert_eq!(layout.mode, Mode::Suite);
        assert_eq!(layout.root, root);
        assert_eq!(layout.other, root.join("other"));
        assert_eq!(layout.programs, Some(root.join("programs")));
        assert_eq!(layout.storage, root.join("storage"));
        Ok(())
    }

    #[test]
    fn override_must_have_the_suite_shape() -> TestResult {
        let tmp = TempTree::new()?
            .dir("empty")?
            .dir("only-other/other")?
            .dir("only-storage/storage")?;
        for folder in ["empty", "only-other", "only-storage", "missing"] {
            let env = Environment {
                install_dir_override: Some(tmp.join(folder)),
                ..Environment::default()
            };
            let result = detect_layout(&env);
            assert!(
                matches!(result, Err(PathsError::NotInstalled)),
                "{folder}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn override_wins_over_the_exe_and_the_repo() -> TestResult {
        let tmp = TempTree::new()?;
        let forced = fake_suite(tmp.join("forced"))?;
        let other = fake_suite(tmp.join("other-install"))?;
        let repo = fake_repo(tmp.path())?;
        let env = Environment {
            debug: true,
            install_dir_override: Some(forced.clone()),
            repo_root: Some(repo),
            ..env_for_exe(launcher_exe(&other))
        };
        assert_eq!(detect_layout(&env)?.root, forced);
        Ok(())
    }

    #[test]
    fn programs_keeps_the_real_folder_name() -> TestResult {
        let tree = TempTree::new()?
            .dir("x/Programs/genslate/launcher")?
            .dir("x/other")?
            .dir("x/storage")?;
        let root = tree.join("x");

        let exe = tree.join("x/Programs/genslate/launcher/launcher.exe");
        let layout = detect_layout(&env_for_exe(exe))?;
        assert_eq!(layout.mode, Mode::Suite);
        assert_eq!(layout.programs, Some(root.join("Programs")));

        let forced = Environment {
            install_dir_override: Some(root.clone()),
            ..Environment::default()
        };
        assert_eq!(
            detect_layout(&forced)?.programs,
            Some(root.join("Programs"))
        );
        Ok(())
    }

    #[test]
    fn profile_dir_never_escapes_storage_users() {
        let layout = Layout {
            mode: Mode::Suite,
            root: PathBuf::from("/r"),
            other: PathBuf::from("/r/other"),
            programs: Some(PathBuf::from("/r/programs")),
            storage: PathBuf::from("/r/storage"),
        };
        let users = PathBuf::from("/r/storage/users");
        for hostile in [
            "",
            ".",
            "..",
            "../x",
            r"..\x",
            "/etc",
            r"\etc",
            r"C:\x",
            "C:",
            "a/b",
            "con",
            "NUL.txt",
            "trailing.",
            r"\\srv\share",
        ] {
            assert_eq!(
                layout.profile_dir(hostile),
                users.join("_invalid"),
                "{hostile:?}"
            );
            assert!(
                matches!(
                    layout.try_profile_dir(hostile),
                    Err(PathsError::InvalidName(ref name)) if name == hostile
                ),
                "{hostile:?}"
            );
        }
    }

    #[test]
    fn ordinary_profile_names_pass_through() -> TestResult {
        let layout = Layout {
            mode: Mode::Suite,
            root: PathBuf::from("/r"),
            other: PathBuf::from("/r/other"),
            programs: None,
            storage: PathBuf::from("/r/storage"),
        };
        for name in [SHARED_PROFILE, "alice", "Bob Smith", "\u{fc}n\u{ef}"] {
            let expected = PathBuf::from("/r/storage/users").join(name);
            assert_eq!(layout.profile_dir(name), expected);
            assert_eq!(layout.try_profile_dir(name)?, expected);
        }
        Ok(())
    }

    #[test]
    fn nothing_known_is_not_installed() {
        assert!(matches!(
            detect_layout(&Environment::default()),
            Err(PathsError::NotInstalled)
        ));
    }

    #[test]
    fn unc_inputs_are_refused_before_anything_else() {
        let unc = PathBuf::from(r"\\fileserver\share\SLATECORE");
        let cases = [
            Environment {
                install_dir_override: Some(unc.clone()),
                ..Environment::default()
            },
            env_for_exe(unc.join(r"programs\genslate\launcher\launcher.exe")),
            Environment {
                debug: true,
                repo_root: Some(unc.clone()),
                ..Environment::default()
            },
        ];
        for env in cases {
            assert!(
                matches!(detect_layout(&env), Err(PathsError::UncPath(_))),
                "{env:?}"
            );
        }
    }

    #[test]
    fn the_unc_error_carries_the_offending_path() {
        let env = Environment {
            install_dir_override: Some(PathBuf::from(r"\\fileserver\share\SLATECORE")),
            ..Environment::default()
        };
        match detect_layout(&env) {
            Err(PathsError::UncPath(path)) => {
                assert_eq!(path, PathBuf::from(r"\\fileserver\share\SLATECORE"));
            }
            other => panic!("expected UncPath, got {other:?}"),
        }
    }

    #[test]
    fn local_verbatim_paths_are_not_unc() {
        let env = Environment {
            exe: Some(PathBuf::from(r"\\?\C:\SLATECORE\launcher.exe")),
            ..Environment::default()
        };
        assert!(matches!(detect_layout(&env), Err(PathsError::NotInstalled)));
    }

    #[test]
    fn profile_dirs_live_under_storage_users() {
        let layout = Layout {
            mode: Mode::Suite,
            root: PathBuf::from("/r"),
            other: PathBuf::from("/r/other"),
            programs: Some(PathBuf::from("/r/programs")),
            storage: PathBuf::from("/r/storage"),
        };
        assert_eq!(
            layout.profile_dir("alice"),
            PathBuf::from("/r/storage/users/alice")
        );
        assert_eq!(layout.name(), Some("r"));
    }
}

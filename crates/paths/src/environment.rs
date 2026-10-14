//! A snapshot of everything path resolution depends on.

use std::env;
use std::path::{self, Path, PathBuf};

use crate::repo::find_repo_root;

/// Forces suite mode with this folder as the install dir (tests, debugging, wrapper scripts).
pub const INSTALL_DIR_ENV: &str = "GENSLATE_INSTALL_DIR";
/// Overrides repository detection in dev mode (useful when the cwd is outside the repo).
pub const REPO_ROOT_ENV: &str = "GENSLATE_REPO_ROOT";

/// Inputs to [`crate::resolve_with`] and [`crate::detect_layout`].
///
/// It holds no operating-system folders: nothing here can lead the launcher outside its
/// install folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    /// Debug build (`cfg!(debug_assertions)`); dev mode is only ever used in debug builds.
    pub debug: bool,
    /// The running executable.
    pub exe: Option<PathBuf>,
    /// [`INSTALL_DIR_ENV`], if set.
    pub install_dir_override: Option<PathBuf>,
    /// Root of the SLATECORE checkout, if the process runs from one.
    pub repo_root: Option<PathBuf>,
}

impl Environment {
    /// Captures the real process environment.
    ///
    /// The executable path, the install folder override and the repository root are
    /// canonicalised without the `\\?\` prefix. On Windows that also turns a mapped network drive
    /// into its UNC form, so a launcher started from `Z:\` that is really `\\server\share` is
    /// refused like any other UNC path.
    pub fn detect() -> Self {
        assemble(Inputs {
            debug: cfg!(debug_assertions),
            exe: env::current_exe().ok(),
            install_dir: non_empty_path(INSTALL_DIR_ENV),
            repo: non_empty_path(REPO_ROOT_ENV),
            cwd: env::current_dir().ok(),
        })
    }
}

/// What [`Environment::detect`] reads from the process, so that [`assemble`] can be tested.
#[derive(Debug, Default)]
struct Inputs {
    debug: bool,
    exe: Option<PathBuf>,
    /// The value of [`INSTALL_DIR_ENV`].
    install_dir: Option<PathBuf>,
    /// The value of [`REPO_ROOT_ENV`].
    repo: Option<PathBuf>,
    cwd: Option<PathBuf>,
}

fn assemble(inputs: Inputs) -> Environment {
    let exe = inputs.exe.map(canonical);
    let repo_root = if inputs.debug {
        detect_repo_root(inputs.repo, exe.as_deref(), inputs.cwd).map(canonical)
    } else {
        None
    };
    Environment {
        debug: inputs.debug,
        exe,
        install_dir_override: inputs.install_dir.map(|dir| canonical(absolute(dir))),
        repo_root,
    }
}

fn non_empty_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Makes a relative path absolute against the current folder (no symlink resolution).
fn absolute(path: PathBuf) -> PathBuf {
    path::absolute(&path).unwrap_or(path)
}

/// Resolves symlinks, `..` and subst or mapped drives, without a `\\?\` prefix; a path that
/// does not exist is returned as given.
fn canonical(path: PathBuf) -> PathBuf {
    dunce::canonicalize(&path).unwrap_or(path)
}

fn detect_repo_root(
    explicit: Option<PathBuf>,
    exe: Option<&Path>,
    cwd: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(explicit) = explicit {
        return find_repo_root(&explicit);
    }
    // `target/debug/<exe>` lives inside the checkout; so does the cwd of `tauri dev`.
    let candidates = [exe.and_then(|exe| exe.parent().map(Path::to_path_buf)), cwd];
    candidates
        .iter()
        .flatten()
        .find_map(|start| find_repo_root(start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_testing::{TempTree, fake_repo, fake_suite};

    #[test]
    fn detect_reports_debug_build_and_exe() {
        let env = Environment::detect();
        assert_eq!(env.debug, cfg!(debug_assertions));
        assert!(env.exe.is_some());
    }

    #[test]
    fn detected_exe_is_absolute() {
        let exe = Environment::detect().exe;
        assert!(exe.is_some_and(|exe| exe.is_absolute()));
    }

    #[test]
    fn detect_finds_the_checkout_this_test_runs_from() {
        // `cargo test` runs from `<repo>/target/debug/deps`, so a debug build sees the repo.
        let env = Environment::detect();
        if env.debug {
            let root = env.repo_root.unwrap_or_default();
            assert!(root.join("Cargo.toml").is_file(), "{}", root.display());
        }
    }

    fn real(path: &Path) -> PathBuf {
        dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    #[test]
    fn the_override_is_canonicalised() -> std::io::Result<()> {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        let messy = root.join("programs").join("..").join("other").join("..");
        let env = assemble(Inputs {
            install_dir: Some(messy),
            ..Inputs::default()
        });
        assert_eq!(env.install_dir_override, Some(real(&root)));
        Ok(())
    }

    #[test]
    fn a_missing_override_stays_as_given_but_absolute() {
        let env = assemble(Inputs {
            install_dir: Some(PathBuf::from("no-such-install-folder")),
            ..Inputs::default()
        });
        let path = env.install_dir_override.unwrap_or_default();
        assert!(path.is_absolute(), "{}", path.display());
        assert!(path.ends_with("no-such-install-folder"));
    }

    #[test]
    fn the_explicit_repo_root_is_canonicalised() -> std::io::Result<()> {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let env = assemble(Inputs {
            debug: true,
            repo: Some(repo.join("programs").join("..")),
            ..Inputs::default()
        });
        assert_eq!(env.repo_root, Some(real(&repo)));
        Ok(())
    }

    #[test]
    fn the_repo_root_found_from_the_cwd_is_canonicalised() -> std::io::Result<()> {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let env = assemble(Inputs {
            debug: true,
            cwd: Some(repo.join("programs").join("..")),
            ..Inputs::default()
        });
        assert_eq!(env.repo_root, Some(real(&repo)));
        Ok(())
    }

    #[test]
    fn the_repo_root_found_from_the_exe_is_canonicalised() -> std::io::Result<()> {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let env = assemble(Inputs {
            debug: true,
            exe: Some(
                repo.join("programs")
                    .join("..")
                    .join("slatecore-launcher.exe"),
            ),
            ..Inputs::default()
        });
        assert_eq!(env.repo_root, Some(real(&repo)));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_roots_are_resolved_before_the_unc_check() -> std::io::Result<()> {
        // The Windows twin of this is a mapped network drive: its canonical form is UNC.
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let root = fake_suite(tmp.path())?;
        std::os::unix::fs::symlink(&repo, tmp.join("repo-link"))?;
        std::os::unix::fs::symlink(&root, tmp.join("install-link"))?;
        let env = assemble(Inputs {
            debug: true,
            install_dir: Some(tmp.join("install-link")),
            repo: Some(tmp.join("repo-link")),
            ..Inputs::default()
        });
        assert_eq!(env.install_dir_override, Some(real(&root)));
        assert_eq!(env.repo_root, Some(real(&repo)));
        Ok(())
    }

    #[test]
    fn an_explicit_repo_root_that_is_not_a_repo_means_no_repo() -> std::io::Result<()> {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let env = assemble(Inputs {
            debug: true,
            repo: Some(tmp.join("not-a-repo")),
            // Neither the cwd nor the exe is consulted once the variable is set.
            cwd: Some(repo.clone()),
            exe: Some(repo.join("slatecore-launcher.exe")),
            ..Inputs::default()
        });
        assert_eq!(env.repo_root, None);
        Ok(())
    }

    #[test]
    fn release_builds_look_for_no_repo() -> std::io::Result<()> {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let env = assemble(Inputs {
            debug: false,
            cwd: Some(repo.clone()),
            exe: Some(repo.join("slatecore-launcher.exe")),
            repo: Some(repo),
            ..Inputs::default()
        });
        assert_eq!(env.repo_root, None);
        Ok(())
    }

    #[test]
    fn environment_variable_names_are_stable() {
        assert_eq!(INSTALL_DIR_ENV, "GENSLATE_INSTALL_DIR");
        assert_eq!(REPO_ROOT_ENV, "GENSLATE_REPO_ROOT");
    }

    #[test]
    fn default_environment_knows_nothing() {
        let env = Environment::default();
        assert!(!env.debug);
        assert_eq!(env.exe, None);
        assert_eq!(env.install_dir_override, None);
        assert_eq!(env.repo_root, None);
    }
}

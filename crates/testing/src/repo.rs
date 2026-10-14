//! Fake SLATECORE layouts: a launcher install folder (suite) and a repository checkout.
//!
//! Both builders create their tree *inside* a folder you pass in (usually a [`crate::TempTree`]),
//! so the same test can build several trees under different roots and compare them.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Files that identify the root of a SLATECORE checkout (kept in lockstep with
/// `genslate-paths`, which has a test for it).
pub const REPO_MARKERS: [&str; 2] = ["turbo.json", "Cargo.toml"];

/// Name of the install folder [`fake_suite`] creates. Real install folders can have any name.
const SUITE_DIR: &str = "SLATECORE";
/// Name of the checkout folder [`fake_repo`] creates.
const REPO_DIR: &str = "repo";
/// The launcher's development mock-up inside a checkout.
const DEV_LAUNCHER: [&str; 3] = ["programs", "desktop", "launcher"];
/// The user folders every profile has.
const PROFILE_FOLDERS: [&str; 6] = [
    "Desktop",
    "Documents",
    "Downloads",
    "Music",
    "Pictures",
    "Videos",
];

/// Builds `<tmp>/SLATECORE`, an empty portable install folder, and returns its path.
///
/// It has the shape `genslate-paths` looks for: `programs/genslate/launcher/`,
/// `programs/portableapps.com/`, `programs/portapps.io/`, `other/` (empty: per-app state is
/// created by the code under test), `storage/users/shared/{Desktop,…}` and `storage/vault/`.
/// Missing parents of `tmp` are created and calling it twice is harmless.
pub fn fake_suite(tmp: impl AsRef<Path>) -> io::Result<PathBuf> {
    let root = tmp.as_ref().join(SUITE_DIR);
    install_dir(&root)?;
    Ok(root)
}

/// Builds `<tmp>/repo`, a checkout with the [`REPO_MARKERS`] and the launcher's development
/// mock-up, and returns its path.
///
/// The mock-up is `programs/desktop/launcher/installDir/` (an install folder as built by
/// [`fake_suite`], used for `programs/` and `storage/`) next to
/// `programs/desktop/launcher/other/`, which dev mode uses for `other/`.
pub fn fake_repo(tmp: impl AsRef<Path>) -> io::Result<PathBuf> {
    let root = tmp.as_ref().join(REPO_DIR);
    fs::create_dir_all(&root)?;
    for marker in REPO_MARKERS {
        fs::write(root.join(marker), "")?;
    }
    let launcher: PathBuf = DEV_LAUNCHER
        .iter()
        .fold(root.clone(), |dir, part| dir.join(part));
    install_dir(&launcher.join("installDir"))?;
    fs::create_dir_all(launcher.join("other"))?;
    Ok(root)
}

/// Creates the folders every install has under `root`.
fn install_dir(root: &Path) -> io::Result<()> {
    let folders = [
        "programs/genslate/launcher",
        "programs/portableapps.com",
        "programs/portapps.io",
        "other",
        "storage/vault",
    ];
    for folder in folders {
        fs::create_dir_all(root.join(folder))?;
    }
    let shared = root.join("storage/users/shared");
    for folder in PROFILE_FOLDERS {
        fs::create_dir_all(shared.join(folder))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TempTree;
    use std::fs;

    const PROFILE_FOLDERS: [&str; 6] = [
        "Desktop",
        "Documents",
        "Downloads",
        "Music",
        "Pictures",
        "Videos",
    ];

    fn assert_install_dir_shape(root: &Path) {
        for dir in [
            "programs/genslate/launcher",
            "programs/portableapps.com",
            "programs/portapps.io",
            "other",
            "storage/vault",
        ] {
            assert!(root.join(dir).is_dir(), "missing {dir}");
        }
        for folder in PROFILE_FOLDERS {
            let dir = root.join("storage/users/shared").join(folder);
            assert!(dir.is_dir(), "missing storage/users/shared/{folder}");
        }
    }

    #[test]
    fn markers_are_turbo_and_cargo() {
        assert_eq!(REPO_MARKERS, ["turbo.json", "Cargo.toml"]);
    }

    #[test]
    fn fake_suite_has_programs_other_and_storage() -> io::Result<()> {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        assert!(root.starts_with(tmp.path()));
        assert_install_dir_shape(&root);
        Ok(())
    }

    #[test]
    fn fake_suite_leaves_per_app_state_to_the_code_under_test() -> io::Result<()> {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        assert_eq!(fs::read_dir(root.join("other"))?.count(), 0);
        Ok(())
    }

    #[test]
    fn fake_repo_has_markers_and_the_dev_mockup() -> io::Result<()> {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        assert!(repo.starts_with(tmp.path()));
        for marker in REPO_MARKERS {
            assert!(repo.join(marker).is_file(), "missing {marker}");
        }
        let mockup = repo.join("programs/desktop/launcher");
        assert_install_dir_shape(&mockup.join("installDir"));
        assert!(mockup.join("other").is_dir());
        Ok(())
    }

    #[test]
    fn builders_can_run_twice_in_one_folder() -> io::Result<()> {
        let tmp = TempTree::new()?;
        assert_eq!(fake_suite(tmp.path())?, fake_suite(tmp.path())?);
        assert_eq!(fake_repo(tmp.path())?, fake_repo(tmp.path())?);
        Ok(())
    }

    #[test]
    fn suite_and_repo_do_not_collide_in_one_folder() -> io::Result<()> {
        let tmp = TempTree::new()?;
        let suite = fake_suite(tmp.path())?;
        let repo = fake_repo(tmp.path())?;
        assert_ne!(suite, repo);
        Ok(())
    }

    #[test]
    fn creates_missing_parent_folders() -> io::Result<()> {
        let tmp = TempTree::new()?;
        let deep = tmp.join("a b/ünï/日本");
        let root = fake_suite(&deep)?;
        assert!(root.starts_with(&deep));
        assert_install_dir_shape(&root);
        Ok(())
    }
}

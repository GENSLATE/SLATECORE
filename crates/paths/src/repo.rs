//! Repository root detection.

use std::path::{Path, PathBuf};

/// Files that together identify the root of a SLATECORE checkout.
const MARKERS: [&str; 2] = ["turbo.json", "Cargo.toml"];

/// Walks up from `start` to the first folder containing the repo markers.
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| MARKERS.iter().all(|marker| dir.join(marker).is_file()))
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_testing::{REPO_MARKERS, TempTree, fake_repo};

    #[test]
    fn markers_match_the_testing_fixture() {
        assert_eq!(MARKERS, REPO_MARKERS);
        assert_eq!(MARKERS, ["turbo.json", "Cargo.toml"]);
    }

    #[test]
    fn finds_root_from_a_nested_folder() -> std::io::Result<()> {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        let nested = repo.join("target").join("debug");
        std::fs::create_dir_all(&nested)?;
        assert_eq!(find_repo_root(&nested).as_deref(), Some(repo.as_path()));
        Ok(())
    }

    #[test]
    fn finds_root_from_the_root_itself() -> std::io::Result<()> {
        let tmp = TempTree::new()?;
        let repo = fake_repo(tmp.path())?;
        assert_eq!(find_repo_root(&repo).as_deref(), Some(repo.as_path()));
        Ok(())
    }

    #[test]
    fn one_marker_alone_is_not_a_checkout() -> std::io::Result<()> {
        let tree = TempTree::new()?.file("only/Cargo.toml", "")?;
        assert_eq!(find_repo_root(&tree.join("only")), None);
        let tree = TempTree::new()?.file("only/turbo.json", "{}")?;
        assert_eq!(find_repo_root(&tree.join("only")), None);
        Ok(())
    }

    #[test]
    fn none_outside_a_checkout() -> std::io::Result<()> {
        let tree = TempTree::new()?.dir("a/b")?;
        assert_eq!(find_repo_root(&tree.join("a/b")), None);
        Ok(())
    }
}

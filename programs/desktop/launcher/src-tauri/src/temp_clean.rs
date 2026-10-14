//! Empties the launcher's own temporary folder (`cache/tmp`, what `TEMP` and `TMP` point at) when
//! it starts, like `web_data` does for the `WebView2` profile.
//!
//! `env_bootstrap` sends the launcher's temporary files into installDir so nothing lands in the
//! PC's `%TEMP%`; without a clean-up they would pile up on the drive, and a plaintext scratch
//! file a program left there would outlive its use. The folder is emptied before the first window
//! exists, after the single-instance plugin has let this launch through (a second launch must not
//! empty the folder the first one is using).
//!
//! Safety rules: only a folder strictly inside the cache folder (no `..`, not the cache folder
//! itself, not a link) is touched; a link inside it is removed as a link and never followed; one
//! entry that cannot be removed (a program started earlier still holds it) is skipped and counted,
//! it does not stop the rest. Programs the launcher started before a restart and still running
//! may lose scratch files in here: that is the price of leaving no trace.

use std::fs;
use std::io;
use std::path::{Component, Path};

/// What a clean-up did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TempReport {
    /// Entries (files, folders, links) removed.
    pub removed: usize,
    /// Entries that could not be removed.
    pub failed: usize,
}

fn refuse(why: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, why)
}

/// `inner` is below `outer`, by whole path components, and is not `outer` itself.
fn is_strictly_inside(inner: &Path, outer: &Path) -> bool {
    inner != outer && inner.starts_with(outer)
}

/// Empties `temp_dir`, which must lie strictly inside `cache_dir`.
///
/// # Errors
///
/// `InvalidInput` when `temp_dir` is not strictly inside `cache_dir`, contains a `..`, or is
/// itself a link, or when either folder resolves (links above it followed) to a place where it
/// is not strictly inside the other; otherwise the error from listing it. A folder that does not exist is a success
/// with nothing removed.
pub fn empty_temp(temp_dir: &Path, cache_dir: &Path) -> io::Result<TempReport> {
    let has_dots = temp_dir
        .components()
        .chain(cache_dir.components())
        .any(|part| part == Component::ParentDir);
    if has_dots {
        return Err(refuse("the temporary folder path contains .."));
    }
    if !is_strictly_inside(temp_dir, cache_dir) {
        return Err(refuse(
            "the temporary folder is not inside the cache folder",
        ));
    }
    let meta = match fs::symlink_metadata(temp_dir) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(TempReport::default()),
        Err(error) => return Err(error),
    };
    if !meta.is_dir() {
        // A link, or a file where the folder should be: not ours to empty or follow.
        return Err(refuse("the temporary folder is not a real folder"));
    }
    // A link anywhere above the folder (a junction for `cache`, say) would make the lexical
    // check above lie: compare where both really are.
    let real_temp = fs::canonicalize(temp_dir)?;
    let real_cache = fs::canonicalize(cache_dir)?;
    if !is_strictly_inside(&real_temp, &real_cache) {
        return Err(refuse(
            "the temporary folder resolves outside the cache folder",
        ));
    }
    let mut report = TempReport::default();
    for entry in fs::read_dir(temp_dir)? {
        let Ok(entry) = entry else {
            report.failed += 1;
            continue;
        };
        let path = entry.path();
        // `file_type` of a directory entry does not follow links.
        let removed = match entry.file_type() {
            Ok(kind) if kind.is_dir() => fs::remove_dir_all(&path),
            // A file, or a link (a folder link is removed as a link, never walked into).
            Ok(_) => fs::remove_file(&path).or_else(|_| fs::remove_dir(&path)),
            Err(error) => Err(error),
        };
        if removed.is_ok() {
            report.removed += 1;
        } else {
            report.failed += 1;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_testing::TempTree;
    use std::error::Error;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn inside_means_below_by_whole_components() {
        assert!(is_strictly_inside(
            Path::new("/a/cache/tmp"),
            Path::new("/a/cache")
        ));
        assert!(!is_strictly_inside(
            Path::new("/a/cache"),
            Path::new("/a/cache")
        ));
        assert!(!is_strictly_inside(
            Path::new("/a/cache2/tmp"),
            Path::new("/a/cache")
        ));
        assert!(!is_strictly_inside(Path::new("/a"), Path::new("/a/cache")));
    }

    #[test]
    fn a_missing_cache_folder_is_an_error_not_a_clean_up() -> TestResult {
        // The folder to empty exists but the folder that should contain it does not resolve.
        let tmp = TempTree::new()?.file("elsewhere/tmp/a.tmp", "x")?;
        let result = empty_temp(&tmp.join("elsewhere/tmp"), &tmp.join("cache"));
        assert!(result.is_err());
        assert!(
            tmp.join("elsewhere/tmp/a.tmp").is_file(),
            "nothing was removed"
        );
        Ok(())
    }

    #[test]
    fn a_folder_that_only_looks_inside_after_resolving_is_refused() -> TestResult {
        // `cache/./x` is not a `..` path, yet a plain prefix check on unresolved names would let
        // a sibling that merely shares the text through: the resolved comparison must not.
        let tmp = TempTree::new()?
            .file("cache/tmp/a.tmp", "x")?
            .file("other/tmp/b.tmp", "y")?;
        let result = empty_temp(&tmp.join("other/tmp"), &tmp.join("cache"));
        assert!(result.is_err());
        assert!(tmp.join("other/tmp/b.tmp").is_file());
        Ok(())
    }

    #[test]
    fn it_empties_files_and_nested_folders_and_keeps_the_folder() -> TestResult {
        let tmp = TempTree::new()?
            .file("cache/tmp/a.tmp", "x")?
            .file("cache/tmp/sub/deep/b.tmp", "y")?
            .dir("cache/tmp/empty")?
            .file("cache/webview2/keep.txt", "profile")?;
        let report = empty_temp(&tmp.join("cache/tmp"), &tmp.join("cache"))?;
        assert_eq!(
            report,
            TempReport {
                removed: 3,
                failed: 0
            }
        );
        assert!(tmp.join("cache/tmp").is_dir(), "the folder itself stays");
        assert_eq!(fs::read_dir(tmp.join("cache/tmp"))?.count(), 0);
        assert!(
            tmp.join("cache/webview2/keep.txt").is_file(),
            "a sibling is not touched"
        );
        Ok(())
    }

    #[test]
    fn a_missing_folder_is_a_success() -> TestResult {
        let tmp = TempTree::new()?.dir("cache")?;
        assert_eq!(
            empty_temp(&tmp.join("cache/tmp"), &tmp.join("cache"))?,
            TempReport::default()
        );
        Ok(())
    }

    #[test]
    fn only_a_folder_strictly_inside_the_cache_is_emptied() -> TestResult {
        let tmp = TempTree::new()?
            .file("cache/tmp/a.tmp", "x")?
            .file("other/precious.txt", "keep")?;
        let cache = tmp.join("cache");
        for bad in [
            tmp.join("other"),
            cache.clone(),
            tmp.path().to_path_buf(),
            cache.join("tmp").join("..").join(".."),
            cache.join("..").join("other"),
        ] {
            let error = empty_temp(&bad, &cache).err();
            assert_eq!(
                error.map(|e| e.kind()),
                Some(io::ErrorKind::InvalidInput),
                "{}",
                bad.display()
            );
        }
        assert!(tmp.join("other/precious.txt").is_file());
        assert!(tmp.join("cache/tmp/a.tmp").is_file());
        Ok(())
    }

    #[test]
    fn a_file_where_the_folder_should_be_is_refused_not_deleted() -> TestResult {
        let tmp = TempTree::new()?.file("cache/tmp", "i am a file")?;
        let result = empty_temp(&tmp.join("cache/tmp"), &tmp.join("cache"));
        assert!(result.is_err());
        assert!(tmp.join("cache/tmp").is_file());
        Ok(())
    }
}

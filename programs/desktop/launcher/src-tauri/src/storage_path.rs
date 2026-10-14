//! Paths the webview may name for vault import and export.
//!
//! The IPC never accepts a raw OS path. A file to encrypt, or a place to decrypt to, is named
//! *relative to `storage/users`* (`shared/Documents/notes.txt`), with `/` or `\` between names.
//! The shell checks every name with the vault's own rules (no `..`, drive letters, device
//! names, reserved characters), refuses anything absolute or UNC, joins it to the folder and
//! then resolves links to prove the result is still inside `storage/users`. A path inside the
//! vault folder or the session folder is therefore impossible by construction.

use std::fs;
use std::path::{Path, PathBuf};

use genslate_vault::{VaultError, VaultPath};

/// The longest path (in characters) accepted.
const MAX_CHARS: usize = 1024;

/// Resolves `input` (relative to `users_root`) to a real path inside it.
///
/// With `must_exist` the target has to exist (an import source); without it only its folder has
/// to (an export destination, which is created by the export).
///
/// # Errors
///
/// [`VaultError::InvalidPath`] for any name or link that leaves `users_root`, and
/// [`VaultError::NotFound`] when a required file or folder is missing.
pub fn resolve_user_path(
    users_root: &Path,
    input: &str,
    must_exist: bool,
) -> Result<PathBuf, VaultError> {
    if input.is_empty() || input.chars().count() > MAX_CHARS {
        return Err(VaultError::InvalidPath("the path is empty or too long"));
    }
    if input.starts_with(['/', '\\']) {
        return Err(VaultError::InvalidPath(
            "give the path relative to storage/users",
        ));
    }
    let mut joined = users_root.to_path_buf();
    for name in input.split(['/', '\\']) {
        // The vault's own component rules: empty, `.`, `..`, reserved characters (this includes
        // the `:` of a drive letter or a stream name), trailing dots and spaces, device names.
        VaultPath::root().join(name)?;
        joined.push(name);
    }

    let base = fs::canonicalize(users_root).map_err(|_| not_found(input))?;
    let resolved = if must_exist {
        fs::canonicalize(&joined).map_err(|_| not_found(input))?
    } else {
        let parent = joined.parent().ok_or_else(|| not_found(input))?;
        let name = joined.file_name().ok_or_else(|| not_found(input))?;
        fs::canonicalize(parent)
            .map_err(|_| not_found(input))?
            .join(name)
    };
    if resolved.starts_with(&base) && resolved != base {
        Ok(resolved)
    } else {
        Err(VaultError::InvalidPath("the path leaves storage/users"))
    }
}

fn not_found(input: &str) -> VaultError {
    VaultError::NotFound(input.to_owned())
}

/// Checks, just before a source is imported, that `input` still resolves to the file it resolved
/// to when the whole request was checked (`first`). Best effort: the vault opens the file a
/// moment later and a link swapped in that gap is not seen by this check, but the gap is now the
/// length of one `open`, not of every import before it.
///
/// # Errors
///
/// The errors of [`resolve_user_path`], and [`VaultError::InvalidPath`] when the answer changed.
pub fn resolve_again(users_root: &Path, input: &str, first: &Path) -> Result<PathBuf, VaultError> {
    let again = resolve_user_path(users_root, input, true)?;
    if again == first {
        Ok(again)
    } else {
        Err(VaultError::InvalidPath("the file changed while importing"))
    }
}

/// Checks, after an export wrote `dest`, that the file really is inside `users_root`: the folder
/// it went into is resolved again, so a link swapped in while the vault was decrypting is caught.
/// A file that landed outside is removed (best effort) and the export reported as refused.
///
/// This narrows, but cannot close, the gap between checking the destination and the vault's own
/// rename (inside the `genslate-vault` crate).
///
/// # Errors
///
/// [`VaultError::InvalidPath`] when the written file is not inside `users_root`.
pub fn verify_written(users_root: &Path, dest: &Path) -> Result<(), VaultError> {
    let inside = fs::canonicalize(users_root).and_then(|base| {
        fs::canonicalize(dest).map(|written| written.starts_with(&base) && written != base)
    });
    match inside {
        Ok(true) => Ok(()),
        Ok(false) => {
            // Best effort: decrypted data has landed where it must not be.
            let _ = fs::remove_file(dest);
            Err(VaultError::InvalidPath("the path leaves storage/users"))
        }
        Err(_) => Err(VaultError::Internal(
            "the exported file could not be checked",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_testing::TempTree;
    use std::error::Error;

    type TestResult = Result<(), Box<dyn Error>>;

    fn tree() -> Result<TempTree, Box<dyn Error>> {
        Ok(TempTree::new()?
            .file("users/shared/Documents/notes.txt", "n")?
            .dir("users/shared/Desktop")?
            .file("vault/vault.a.gvh", "secret")?
            .file("outside.txt", "x")?)
    }

    fn invalid(result: Result<PathBuf, VaultError>) -> bool {
        matches!(result, Err(VaultError::InvalidPath(_)))
    }

    #[test]
    fn a_relative_name_resolves_inside_users() -> TestResult {
        let tmp = tree()?;
        let users = tmp.join("users");
        let file = resolve_user_path(&users, "shared/Documents/notes.txt", true)?;
        assert_eq!(
            file,
            fs::canonicalize(users.join("shared/Documents/notes.txt"))?
        );
        let same = resolve_user_path(&users, r"shared\Documents\notes.txt", true)?;
        assert_eq!(file, same);
        Ok(())
    }

    #[test]
    fn dot_dot_absolute_drive_and_unc_are_refused() -> TestResult {
        let tmp = tree()?;
        let users = tmp.join("users");
        for bad in [
            "../vault/vault.a.gvh",
            "shared/../../outside.txt",
            "shared/Documents/../../../outside.txt",
            "/etc/passwd",
            r"\\server\share\x.txt",
            r"C:\Windows\win.ini",
            "C:win.ini",
            "shared//Documents",
            "shared/./Documents/notes.txt",
            "shared/Documents/notes.txt:stream",
            "shared/con",
            "shared/NUL.txt",
            "shared/trailing.",
            "shared/Documents/a*b",
        ] {
            assert!(invalid(resolve_user_path(&users, bad, true)), "{bad}");
            assert!(invalid(resolve_user_path(&users, bad, false)), "{bad}");
        }
        assert!(invalid(resolve_user_path(&users, "", true)));
        Ok(())
    }

    #[test]
    fn a_missing_import_source_is_not_found() -> TestResult {
        let tmp = tree()?;
        let users = tmp.join("users");
        assert!(matches!(
            resolve_user_path(&users, "shared/Documents/missing.txt", true),
            Err(VaultError::NotFound(_))
        ));
        Ok(())
    }

    #[test]
    fn an_export_destination_needs_only_its_folder() -> TestResult {
        let tmp = tree()?;
        let users = tmp.join("users");
        let dest = resolve_user_path(&users, "shared/Desktop/out.txt", false)?;
        assert!(dest.ends_with("out.txt"));
        assert!(!dest.exists());
        assert!(matches!(
            resolve_user_path(&users, "shared/NoSuchFolder/out.txt", false),
            Err(VaultError::NotFound(_))
        ));
        Ok(())
    }

    #[test]
    fn resolving_again_accepts_the_same_file_and_refuses_another() -> TestResult {
        let tmp = tree()?.file("users/shared/Documents/other.txt", "o")?;
        let users = tmp.join("users");
        let first = resolve_user_path(&users, "shared/Documents/notes.txt", true)?;
        assert_eq!(
            resolve_again(&users, "shared/Documents/notes.txt", &first)?,
            first
        );
        let other = resolve_user_path(&users, "shared/Documents/other.txt", true)?;
        assert!(invalid(resolve_again(
            &users,
            "shared/Documents/notes.txt",
            &other
        )));
        Ok(())
    }

    #[test]
    fn resolving_again_reports_a_file_that_vanished() -> TestResult {
        let tmp = tree()?;
        let users = tmp.join("users");
        let first = resolve_user_path(&users, "shared/Documents/notes.txt", true)?;
        fs::remove_file(&first)?;
        assert!(matches!(
            resolve_again(&users, "shared/Documents/notes.txt", &first),
            Err(VaultError::NotFound(_))
        ));
        Ok(())
    }

    #[test]
    fn a_file_written_inside_users_passes_the_after_check() -> TestResult {
        let tmp = tree()?;
        let users = tmp.join("users");
        let dest = users.join("shared/Desktop/out.txt");
        fs::write(&dest, "decrypted")?;
        verify_written(&users, &dest)?;
        assert!(dest.exists(), "a good export is left alone");
        Ok(())
    }

    #[test]
    fn a_file_that_landed_outside_users_is_removed_and_refused() -> TestResult {
        let tmp = tree()?;
        let users = tmp.join("users");
        let escaped = tmp.join("outside.txt");
        assert!(escaped.exists());
        assert!(invalid(
            verify_written(&users, &escaped).map(|()| PathBuf::new())
        ));
        assert!(!escaped.exists(), "the stray plaintext is removed");
        Ok(())
    }

    #[test]
    fn a_missing_written_file_is_an_error_not_a_pass() -> TestResult {
        let tmp = tree()?;
        let users = tmp.join("users");
        let result = verify_written(&users, &users.join("shared/Desktop/nothing.txt"));
        assert!(result.is_err());
        Ok(())
    }
}

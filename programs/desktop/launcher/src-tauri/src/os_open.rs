//! Opens a folder in File Explorer or a file in the program Windows associates with it.
//!
//! This stands in for the opener plugin, which stays unregistered (its Rust side reaches the
//! registry-backed shell association code and its JS side lets the webview open things). Here
//! only the shell calls it, with a path it built itself from an id or a name, and it is the
//! same as double-clicking the item in Explorer.
//!
//! **Opening leaves traces on the host.** Explorer and the program it starts are outside the
//! launcher's control (`portability-no-trace.md`, "State the limits"): the item shows up in
//! Windows' Recent Items and Jump Lists, and the program keeps its own most-recently-used list,
//! autosave and temporary files wherever it likes. Explorer usually hands the item to the
//! shell that is already running, so the program starts with the PC's own environment and not
//! with the launcher's redirected `TEMP`. For decrypted vault files this means plaintext copies
//! and file names can end up on the PC. The vault UI has to say so before it opens a file (a
//! follow-up for the frontend); `open_policy` refuses the types that could run code.
//!
//! Only the shell calls this and only with an absolute path: [`explorer_argument`] builds the
//! command line, `sys::spawn_explorer` starts Explorer.

use std::ffi::OsString;
use std::io;
use std::path::Path;

use crate::sys;

fn invalid(why: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, why)
}

/// The command line Explorer gets after its own name: `path` in double quotes.
///
/// The path must be absolute, so Explorer can never read it as one of its own switches
/// (`/select,`, `/e,` ...). It is quoted whole so a name with a comma, which Explorer reads as a
/// separator, opens as the one item it is.
///
/// # Errors
///
/// `InvalidInput` for a relative path, and for a path with a `"` in it (not a valid Windows file
/// name, and the one character that could end the quoting early).
pub fn explorer_argument(path: &Path) -> io::Result<OsString> {
    if !path.is_absolute() {
        return Err(invalid("only an absolute path can be opened"));
    }
    if path.to_string_lossy().contains('"') {
        return Err(invalid("a path cannot contain a double quote"));
    }
    let mut argument = OsString::with_capacity(path.as_os_str().len() + 2);
    argument.push("\"");
    argument.push(path.as_os_str());
    argument.push("\"");
    Ok(argument)
}

/// Opens `path` (a folder or a file) the way a double click in Explorer would.
///
/// # Errors
///
/// `InvalidInput` for a relative path, otherwise the error from starting Explorer. Whether the
/// item then opens is up to Windows (a file with no associated program shows its "Open with"
/// dialog).
pub fn open_path(path: &Path) -> io::Result<()> {
    sys::spawn_explorer(&explorer_argument(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute path on whatever host runs the tests.
    fn absolute(tail: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(tail)
    }

    #[test]
    fn a_relative_path_is_refused_before_anything_starts() {
        let error = open_path(Path::new("documents")).err();
        assert_eq!(error.map(|e| e.kind()), Some(io::ErrorKind::InvalidInput));
        assert!(
            explorer_argument(&absolute("x")).is_ok(),
            "an absolute path is fine"
        );
        assert!(explorer_argument(Path::new("..")).is_err());
        assert!(explorer_argument(Path::new("")).is_err());
    }

    #[test]
    fn the_whole_path_is_quoted_even_with_commas_and_no_spaces() -> Result<(), io::Error> {
        let path = absolute("a,b,c").join("report,final.txt");
        let argument = explorer_argument(&path)?;
        let expected = format!("\"{}\"", path.display());
        assert_eq!(argument, OsString::from(&expected));
        assert!(expected.starts_with('"') && expected.ends_with('"'));
        Ok(())
    }

    #[test]
    fn spaces_and_unicode_are_kept_inside_the_quotes() -> Result<(), io::Error> {
        let path = absolute("My Folder").join("archivo \u{e9}t\u{e9}.txt");
        let argument = explorer_argument(&path)?;
        assert_eq!(argument, OsString::from(format!("\"{}\"", path.display())));
        Ok(())
    }

    #[test]
    fn a_quote_in_the_path_is_refused() {
        let path = absolute("evil\" /e,\"C:");
        let error = explorer_argument(&path).err();
        assert_eq!(error.map(|e| e.kind()), Some(io::ErrorKind::InvalidInput));
    }

    #[test]
    fn a_folder_with_a_trailing_separator_keeps_it_inside_the_quotes() -> Result<(), io::Error> {
        let mut text = absolute("folder").into_os_string();
        text.push(std::path::MAIN_SEPARATOR_STR);
        let argument = explorer_argument(Path::new(&text))?;
        let shown = argument.to_string_lossy().into_owned();
        assert!(
            shown.ends_with(&format!("{}\"", std::path::MAIN_SEPARATOR)),
            "{shown}"
        );
        Ok(())
    }
}

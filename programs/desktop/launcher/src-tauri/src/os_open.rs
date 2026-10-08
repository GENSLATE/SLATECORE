//! Opens a folder in File Explorer or a file in the program Windows associates with it.
//!
//! This stands in for the opener plugin, which stays unregistered (its Rust side reaches the
//! registry-backed shell association code and its JS side lets the webview open things). Here
//! only the shell calls it, with a path it built itself from an id or a name, and it is the
//! same as double-clicking the item in Explorer: nothing is written anywhere.

use std::io;
use std::path::Path;

/// The path must be absolute, so Explorer can never read it as one of its own switches.
fn validate(path: &Path) -> io::Result<()> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "only an absolute path can be opened",
        ))
    }
}

/// Opens `path` (a folder or a file) the way a double click in Explorer would.
///
/// # Errors
///
/// `InvalidInput` for a relative path, otherwise the error from starting Explorer. Whether the
/// item then opens is up to Windows (a file with no associated program shows its "Open with"
/// dialog).
pub fn open_path(path: &Path) -> io::Result<()> {
    validate(path)?;
    imp::open(path)
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    use crate::env_bootstrap::{Env, child_env};

    /// `%SystemRoot%\explorer.exe`, named in full so no other `explorer.exe` can be picked up.
    fn explorer() -> io::Result<PathBuf> {
        std::env::var_os("SystemRoot")
            .map(|root| PathBuf::from(root).join("explorer.exe"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "SystemRoot is not set"))
    }

    pub fn open(path: &Path) -> io::Result<()> {
        // A program that opens the item may embed `WebView2`: it must not inherit the
        // launcher's `WEBVIEW2_*` variables and share (and lock) the launcher's data folder.
        let inherited: Env = std::env::vars().collect();
        // Explorer reports failure through its exit code even on success, so it is not awaited.
        Command::new(explorer()?)
            .arg(path)
            .env_clear()
            .envs(child_env(&inherited))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(drop)
    }
}

/// Only Windows ships; this keeps the crate building (and its tests running) on other hosts.
#[cfg(not(windows))]
mod imp {
    use std::io;
    use std::path::Path;

    pub fn open(_path: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_path_is_refused_before_anything_starts() {
        let error = open_path(Path::new("documents")).err();
        assert_eq!(error.map(|e| e.kind()), Some(io::ErrorKind::InvalidInput));
    }
}

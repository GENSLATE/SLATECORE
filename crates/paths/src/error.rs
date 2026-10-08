//! Errors raised while resolving paths.

use std::io::{self, ErrorKind};
use std::path::PathBuf;

/// Why the install folder could not be found, used or created.
///
/// There is deliberately no "use another location" variant: when the install folder is no
/// good, the launcher says why and exits instead of writing to the PC's user folders.
#[derive(Debug, thiserror::Error)]
pub enum PathsError {
    /// The executable is not inside an install folder and this is not a development checkout.
    #[error(
        "SLATECORE LAUNCHER could not find its install folder. Start it from inside the folder \
         that holds `programs`, `other` and `storage`."
    )]
    NotInstalled,
    /// The install folder exists but cannot be written (read-only media, write-protected
    /// drive, missing permission). The path is the folder that could not be created or written.
    #[error(
        "{} is not writable. SLATECORE LAUNCHER keeps all of its files in its own folder and \
         never uses another location. Move the folder to a drive you can write to.",
        .0.display()
    )]
    NotWritable(PathBuf),
    /// The install folder is on a network share (`\\server\share`). The path is the offender.
    #[error(
        "{} is a network (UNC) path. SLATECORE LAUNCHER cannot run from a network share. \
         Copy it to a local or USB drive and start it from there.",
        .0.display()
    )]
    UncPath(PathBuf),
    /// An app name that is not lowercase kebab-case (`launcher`, `explorer`), or a profile name
    /// that is not one plain folder name.
    #[error(
        "invalid name {0:?}: app names use lowercase letters, digits and '-' (not at the end, \
         not a Windows device name); profile names are one plain folder name"
    )]
    InvalidName(String),
    /// Any other file system failure.
    #[error("could not access {}: {source}", path.display())]
    Io {
        /// The file or folder involved.
        path: PathBuf,
        /// What the OS said.
        #[source]
        source: io::Error,
    },
}

impl PathsError {
    /// Sorts an I/O failure on `path` into [`PathsError::NotWritable`] (permission or
    /// read-only medium) or [`PathsError::Io`].
    pub(crate) fn from_io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        let path = path.into();
        match source.kind() {
            ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => Self::NotWritable(path),
            _ => Self::Io { path, source },
        }
    }

    /// `true` when the location exists but may not be written.
    pub fn is_not_writable(&self) -> bool {
        matches!(self, Self::NotWritable(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};
    use std::path::Path;

    #[test]
    fn permission_denied_means_not_writable() {
        let error = PathsError::from_io("/x/other", Error::from(ErrorKind::PermissionDenied));
        assert!(matches!(&error, PathsError::NotWritable(path) if path == Path::new("/x/other")));
        assert!(error.is_not_writable());
    }

    #[test]
    fn read_only_filesystem_means_not_writable() {
        let error = PathsError::from_io("/x", Error::from(ErrorKind::ReadOnlyFilesystem));
        assert!(matches!(error, PathsError::NotWritable(_)));
    }

    #[test]
    fn other_io_errors_keep_their_source() {
        let error = PathsError::from_io("/x", Error::from(ErrorKind::NotFound));
        assert!(!error.is_not_writable());
        match error {
            PathsError::Io { path, source } => {
                assert_eq!(path, Path::new("/x"));
                assert_eq!(source.kind(), ErrorKind::NotFound);
            }
            other => panic!("expected Io, got {other:?}"),
        }
    }

    #[test]
    fn messages_tell_the_user_what_to_do() {
        let not_installed = PathsError::NotInstalled.to_string();
        assert!(
            not_installed.contains("SLATECORE LAUNCHER"),
            "{not_installed}"
        );
        assert!(not_installed.contains("programs"), "{not_installed}");

        let not_writable = PathsError::NotWritable(PathBuf::from("E:/SLATECORE/other")).to_string();
        assert!(
            not_writable.contains("E:/SLATECORE/other"),
            "{not_writable}"
        );
        assert!(not_writable.contains("writable"), "{not_writable}");

        let unc = PathsError::UncPath(PathBuf::from(r"\\srv\share")).to_string();
        assert!(unc.contains(r"\\srv\share"), "{unc}");
        assert!(unc.contains("network"), "{unc}");

        let name = PathsError::InvalidName("Bad Name".to_owned()).to_string();
        assert!(name.contains("Bad Name"), "{name}");
    }

    #[test]
    fn io_error_message_names_the_path_and_the_cause() {
        let error = PathsError::from_io("/x/y", Error::other("disk on fire"));
        let message = error.to_string();
        assert!(message.contains("/x/y"), "{message}");
        assert!(message.contains("disk on fire"), "{message}");
    }
}

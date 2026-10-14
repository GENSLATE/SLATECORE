//! [`LauncherError`]: every failure the launcher core can report.

// cspell:ignore rposition
use std::path::{Path, PathBuf};

/// Errors from settings, catalog, launching and the command registry.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LauncherError {
    /// A file could not be read.
    #[error("could not read {path}: {source}")]
    Read {
        /// The file.
        path: PathBuf,
        /// What the OS said.
        #[source]
        source: std::io::Error,
    },
    /// A file or folder could not be written.
    #[error("could not write {path}: {source}")]
    Write {
        /// The file or folder.
        path: PathBuf,
        /// What the OS said.
        #[source]
        source: std::io::Error,
    },
    /// A file was read but its content is not valid.
    #[error("{path} is invalid: {message}")]
    Parse {
        /// The file.
        path: PathBuf,
        /// What is wrong, with a line number when the parser knows it.
        message: String,
    },
    /// No app has this id.
    #[error("there is no app {0:?}")]
    UnknownApp(String),
    /// The app is known (metadata, reserved name) but its folder is not in `programs/`.
    #[error("{0} is not installed")]
    NotInstalled(String),
    /// The app is listed but cannot be started: its program is missing or its manifest is
    /// damaged. It shows in the "Unavailable" group.
    #[error("{app} cannot be started: {reason}")]
    Unavailable {
        /// The app's display name.
        app: String,
        /// Why, in a few words.
        reason: &'static str,
    },
    /// A program to launch resolves outside `programs/`.
    #[error("{0} is not inside an allowed programs folder")]
    OutsideRoot(PathBuf),
    /// The operating system refused to start the program.
    #[error("could not start {path}: {source}")]
    Spawn {
        /// The program.
        path: PathBuf,
        /// What the OS said.
        #[source]
        source: std::io::Error,
    },
    /// No command has this id.
    #[error("there is no command {0:?}")]
    UnknownAction(String),
    /// A command was given parameters its spec does not allow.
    #[error("/{action}: {message}")]
    InvalidParams {
        /// The command id.
        action: String,
        /// What is wrong.
        message: String,
    },
    /// The file watcher could not start.
    #[error("file watching failed: {0}")]
    Watch(#[from] notify::Error),
}

impl LauncherError {
    /// Stable machine-readable kind (sent to the UI next to the message).
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Read { .. } => "read",
            Self::Write { .. } => "write",
            Self::Parse { .. } => "parse",
            Self::UnknownApp(_) => "unknown-app",
            Self::NotInstalled(_) => "not-installed",
            Self::Unavailable { .. } => "unavailable",
            Self::OutsideRoot(_) => "outside-root",
            Self::Spawn { .. } => "spawn",
            Self::UnknownAction(_) => "unknown-action",
            Self::InvalidParams { .. } => "invalid-params",
            Self::Watch(_) => "watch",
        }
    }
}

/// How a path is written in the log and in issue texts: the part below the last `programs`
/// folder (`portapps.io/brave-portable`), or just the file name when it is not under one. Never
/// a drive letter or a user folder: the log is persisted and travels with the drive.
pub(crate) fn path_label(path: &Path) -> String {
    let text = path.to_string_lossy();
    let segments = segments(&text);
    segments
        .iter()
        .rposition(|segment| segment.eq_ignore_ascii_case("programs"))
        .and_then(|index| segments.get(index + 1..))
        .filter(|below| !below.is_empty())
        .map_or_else(|| last_segment(&segments), |below| below.join("/"))
}

/// The last component of a path (either slash), without its folders.
pub(crate) fn file_label(path: &Path) -> String {
    last_segment(&segments(&path.to_string_lossy()))
}

fn segments(text: &str) -> Vec<&str> {
    text.split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// The last segment, unless it is only a drive (`C:`).
fn last_segment(segments: &[&str]) -> String {
    segments
        .iter()
        .rev()
        .find(|segment| !segment.ends_with(':'))
        .map_or_else(String::new, |segment| (*segment).to_owned())
}

pub(crate) fn read_error(path: &Path) -> impl FnOnce(std::io::Error) -> LauncherError {
    let path = path.to_path_buf();
    move |source| LauncherError::Read { path, source }
}

pub(crate) fn write_error(path: &Path) -> impl FnOnce(std::io::Error) -> LauncherError {
    let path = path.to_path_buf();
    move |source| LauncherError::Write { path, source }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_labelled_below_programs_or_by_file_name() {
        let label = |path: &str| path_label(Path::new(path));
        assert_eq!(
            label("S:\\SLATECORE\\programs\\portapps.io\\brave-portable"),
            "portapps.io/brave-portable"
        );
        assert_eq!(
            label("/media/me/USB/SLATECORE/Programs/genslate/x/x.exe"),
            "genslate/x/x.exe"
        );
        assert_eq!(label("D:\\a\\programs\\b\\programs\\c.exe"), "c.exe");
        // Not under programs/: the file name only.
        assert_eq!(label("E:\\Users\\me\\cache\\icons\\ab12.png"), "ab12.png");
        assert_eq!(label("/home/me/config/settings.toml"), "settings.toml");
        // The programs folder itself, a bare drive and nothing at all leak nothing.
        assert_eq!(label("S:\\SLATECORE\\programs"), "programs");
        assert_eq!(label("C:\\"), "");
        assert_eq!(label(""), "");
        assert_eq!(
            file_label(Path::new("S:\\SLATECORE\\programs\\a\\b.exe")),
            "b.exe"
        );
    }

    #[test]
    fn no_label_carries_a_drive_or_a_root() {
        for path in [
            "S:\\SLATECORE\\programs\\portapps.io\\x",
            "C:\\Users\\me\\AppData\\settings.toml",
            "\\\\server\\share\\programs\\a",
            "/mnt/usb/programs/a/b",
        ] {
            for label in [path_label(Path::new(path)), file_label(Path::new(path))] {
                assert!(
                    !label.contains(':') && !label.starts_with(['/', '\\']),
                    "{path} -> {label}"
                );
            }
        }
    }
}

//! [`LauncherError`]: every failure the launcher core can report.

use std::path::PathBuf;

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

pub(crate) fn read_error(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> LauncherError {
    let path = path.to_path_buf();
    move |source| LauncherError::Read { path, source }
}

pub(crate) fn write_error(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> LauncherError {
    let path = path.to_path_buf();
    move |source| LauncherError::Write { path, source }
}

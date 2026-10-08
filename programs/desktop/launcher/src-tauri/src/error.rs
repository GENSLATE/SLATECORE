//! [`AppError`]: every error a launcher command can send to the webview, as `{ kind, message }`.
//!
//! The message is written for a person and never holds an absolute path: the launcher core's own
//! errors name the file they stopped on, and a path is where the drive letter, the user's name
//! and the folder layout leak. Only the file name survives here, in the UI and in the log (the
//! wording is in `error_text`).

use genslate_launcher_core::LauncherError;

use crate::error_text::launcher_message;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Errors from commands and windows.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AppError {
    #[error("window `{0}` does not exist")]
    MissingWindow(&'static str),
    #[error("{0}")]
    InvalidArgument(String),
    #[error(transparent)]
    Launcher(#[from] LauncherError),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    /// The operating system refused to open a file or folder.
    #[error("could not open it: {0}")]
    Open(String),
}

impl AppError {
    /// Stable machine-readable kind for the UI.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::MissingWindow(_) => "missing-window",
            Self::InvalidArgument(_) => "invalid-argument",
            Self::Launcher(error) => error.kind(),
            Self::Tauri(_) => "tauri",
            Self::Open(_) => "open",
        }
    }

    /// The text the UI shows and the log keeps: no folder names, only file names.
    pub fn message(&self) -> String {
        match self {
            Self::Launcher(error) => launcher_message(error),
            other => other.to_string(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.message())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_serialise_as_kind_and_message() -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(AppError::InvalidArgument("nope".to_owned()))?;
        assert_eq!(json["kind"], "invalid-argument");
        assert_eq!(json["message"], "nope");
        Ok(())
    }

    #[test]
    fn launcher_errors_keep_their_kind_and_lose_their_folders() {
        let error = AppError::from(LauncherError::NotInstalled("Explorer".to_owned()));
        assert_eq!(error.kind(), "not-installed");
        assert_eq!(error.message(), "Explorer is not installed");
    }
}

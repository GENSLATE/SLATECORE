//! Start-up failures and what the user is told about them.
//!
//! A GUI program on Windows has no console, so printing to stderr shows nothing: the message
//! goes into a message box (`dialog`). The wording is chosen here, free of any GUI code.

use crate::brand::PRODUCT;
use crate::runtime::RuntimeError;

/// Why the launcher cannot start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fatal {
    /// Started "as administrator": `WebView2` ignores the launcher's environment then.
    Elevated,
    /// The install folder is missing, unwritable or on a network share. Holds the message of
    /// `genslate_paths::PathsError`, which already says what to do.
    Paths(String),
    /// No `WebView2` runtime on the PC or on the drive (the PC's one may be too old).
    NoRuntime { host_too_old: Option<String> },
    /// A runtime was chosen but `WebView2` would not start with it.
    RuntimeFailed,
    /// Anything else that stopped start-up.
    Startup(String),
}

impl From<RuntimeError> for Fatal {
    fn from(error: RuntimeError) -> Self {
        match error {
            RuntimeError::NoneAvailable { host } => Self::NoRuntime {
                host_too_old: host.map(|version| version.to_string()),
            },
            RuntimeError::BadVersion(text) => {
                Self::Startup(format!("bad runtime version {text:?}"))
            }
        }
    }
}

/// A message box's title and text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub title: &'static str,
    pub body: String,
}

/// The message box for `fatal`.
pub fn message(fatal: &Fatal) -> Message {
    let body = match fatal {
        Fatal::Elevated => format!(
            "{PRODUCT} cannot run as administrator. Windows hides its settings from \
             administrator programs, so the launcher could not keep its files on this drive.\n\n\
             Close it and start it again normally (not with \"Run as administrator\")."
        ),
        Fatal::Paths(text) => text.clone(),
        Fatal::NoRuntime { host_too_old } => {
            let host = host_too_old.as_ref().map_or_else(
                || "This PC has no Microsoft Edge WebView2 runtime".to_owned(),
                |version| {
                    format!("The Microsoft Edge WebView2 runtime on this PC ({version}) is too old")
                },
            );
            format!(
                "{host}, and none is bundled on this drive.\n\n\
                 Install or update the WebView2 runtime from Microsoft, or add the SLATECORE \
                 WebView2 runtime package to this drive (other\\shared\\webview2), then start \
                 {PRODUCT} again."
            )
        }
        Fatal::RuntimeFailed => format!(
            "The WebView2 runtime could not be started.\n\n\
             If {PRODUCT} was started as administrator, close it and start it normally."
        ),
        Fatal::Startup(text) => format!("{PRODUCT} could not start.\n\n{text}"),
    };
    Message {
        title: PRODUCT,
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Version, select};
    use std::error::Error;

    #[test]
    fn fatal_message_used_when_no_runtime() -> Result<(), Box<dyn Error>> {
        let error = select(None, None, Version::new(120, 0, 0, 0))
            .err()
            .ok_or("expected an error")?;
        let message = message(&Fatal::from(error));
        assert_eq!(message.title, "SLATECORE LAUNCHER");
        assert!(message.body.contains("WebView2"), "{}", message.body);
        assert!(message.body.contains("no Microsoft Edge WebView2 runtime"));
        assert!(message.body.contains("other\\shared\\webview2"));

        let old = select(
            Some(Version::new(100, 0, 1, 2)),
            None,
            Version::new(120, 0, 0, 0),
        )
        .err()
        .ok_or("expected an error")?;
        assert!(message_of(old).contains("100.0.1.2"));
        Ok(())
    }

    fn message_of(error: RuntimeError) -> String {
        message(&Fatal::from(error)).body
    }

    #[test]
    fn the_runtime_failure_message_mentions_administrator() {
        let text = message(&Fatal::RuntimeFailed).body;
        assert!(text.contains("administrator"), "{text}");
    }

    #[test]
    fn a_paths_failure_shows_the_paths_crates_own_words() {
        let text = message(&Fatal::Paths("E:\\X is not writable.".to_owned())).body;
        assert_eq!(text, "E:\\X is not writable.");
    }
}

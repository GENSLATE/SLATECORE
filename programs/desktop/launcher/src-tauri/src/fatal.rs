//! Start-up failures and what the user is told about them.
//!
//! A GUI program on Windows has no console, so printing to stderr shows nothing: the message
//! goes into a message box (`dialog`). The wording is chosen here, free of any GUI code.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::brand::PRODUCT;
use crate::runtime::RuntimeError;

/// The step of `setup` (run by Tauri once the event loop is up) that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    /// The launcher window could not be created.
    Window,
    /// The tray icon could not be created. Fatal on purpose: with no icon and the window hidden
    /// nothing can bring the launcher back, so it would sit in memory unreachable.
    Tray,
    /// The first placement and show of the window failed.
    Show,
}

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
    /// A step of `setup` failed. Tauri turns an error returned from `setup` into a panic, and
    /// with `panic = "abort"` in a program with no console that is a silent exit, so the shell
    /// shows this instead. It carries no detail: the detail (which can hold a path) goes to the
    /// log.
    Setup(SetupStep),
    /// A panic reached the panic hook: the last line of defence, generic on purpose.
    Crashed,
    /// Anything else that stopped start-up.
    Startup(String),
}

impl From<SetupStep> for Fatal {
    fn from(step: SetupStep) -> Self {
        Self::Setup(step)
    }
}

/// `true` for the first caller only. The panic hook uses it so that a second panic (or a panic
/// inside the dialog itself) does not show a second box or recurse.
pub fn first_call(flag: &AtomicBool) -> bool {
    !flag.swap(true, Ordering::SeqCst)
}

/// Runs `work` on a fresh thread and waits at most `bound` for it. `true` when it finished,
/// `false` when it was still running at the deadline or panicked. The thread is left behind in
/// the first case: the caller ends the process.
pub fn bounded(bound: Duration, work: impl FnOnce() + Send + 'static) -> bool {
    let (done, finished) = mpsc::channel();
    let spawned = thread::Builder::new()
        .name("bounded".into())
        .spawn(move || {
            work();
            // The receiver may have given up already.
            let _ = done.send(());
        });
    spawned.is_ok() && finished.recv_timeout(bound).is_ok()
}

/// `file:line` of a panic with the folders cut off (they hold the build machine's layout).
pub fn panic_place(file: &str, line: u32) -> String {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file); // cspell:ignore rsplit
    format!("{name}:{line}")
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
        Fatal::Setup(step) => {
            let what = match step {
                SetupStep::Window => "Its window could not be created.".to_owned(),
                SetupStep::Tray => format!(
                    "Its tray icon could not be created. Without it {PRODUCT} could not be \
                     shown again once hidden, so it stops here."
                ),
                SetupStep::Show => "Its window could not be shown.".to_owned(),
            };
            format!(
                "{PRODUCT} could not start.\n\n{what}\n\nStart it again. If it keeps \
                 happening, the log in the launcher's logs folder says why."
            )
        }
        Fatal::Crashed => format!(
            "{PRODUCT} stopped unexpectedly and will close.\n\nStart it again. If it keeps \
             happening, the log in the launcher's logs folder says why."
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
    use std::time::Instant;

    #[test]
    fn bounded_work_that_finishes_reports_true() {
        assert!(bounded(Duration::from_secs(5), || {}));
    }

    #[test]
    fn bounded_work_that_hangs_is_given_up_at_the_deadline() {
        let start = Instant::now();
        let finished = bounded(Duration::from_millis(100), || {
            thread::sleep(Duration::from_secs(5));
        });
        assert!(!finished);
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "waited for the work"
        );
    }

    #[test]
    fn bounded_work_that_panics_is_not_waited_for() {
        let start = Instant::now();
        let finished = bounded(Duration::from_secs(5), || panic!("on purpose"));
        assert!(!finished);
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn a_panic_place_keeps_the_file_name_only() {
        assert_eq!(
            panic_place("src/vault_commands.rs", 12),
            "vault_commands.rs:12"
        );
        assert_eq!(panic_place(r"C:\Users\jane\proj\src\lib.rs", 7), "lib.rs:7");
        assert_eq!(panic_place("lib.rs", 1), "lib.rs:1");
    }
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

    #[test]
    fn a_failed_setup_step_becomes_a_message_with_no_path_and_no_detail() {
        for step in [SetupStep::Window, SetupStep::Tray, SetupStep::Show] {
            let fatal = Fatal::from(step);
            assert_eq!(fatal, Fatal::Setup(step));
            let text = message(&fatal).body;
            assert!(
                text.contains("SLATECORE LAUNCHER could not start"),
                "{text}"
            );
            assert!(text.contains("Start it again"), "{text}");
            assert!(!text.contains('\\') && !text.contains(":/"), "{text}");
        }
        let tray = message(&Fatal::from(SetupStep::Tray)).body;
        assert!(tray.contains("tray icon"), "{tray}");
        assert!(tray.contains("stops here"), "{tray}");
        let window = message(&Fatal::from(SetupStep::Window)).body;
        assert!(window.contains("window could not be created"), "{window}");
        assert_ne!(window, message(&Fatal::from(SetupStep::Show)).body);
    }

    #[test]
    fn the_crash_message_is_generic() {
        let text = message(&Fatal::Crashed).body;
        assert!(text.contains("stopped unexpectedly"), "{text}");
        assert!(!text.contains('\\') && !text.contains(":/"), "{text}");
    }

    #[test]
    fn the_panic_hook_shows_its_box_once() {
        let flag = AtomicBool::new(false);
        assert!(first_call(&flag));
        assert!(
            !first_call(&flag),
            "a second panic, or one inside the box, shows nothing"
        );
        assert!(!first_call(&flag));
    }
}

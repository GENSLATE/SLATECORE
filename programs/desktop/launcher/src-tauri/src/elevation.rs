//! The launcher refuses to run elevated: `WebView2` ignores `WEBVIEW2_*` variables (the data
//! folder and the bundled runtime) in an elevated process, which would put its files on the PC.
//!
//! There is no safe std or Tauri API for the question "is this process elevated", and `unsafe`
//! is not allowed outside `env_bootstrap`. So the shell asks the system's own `whoami /groups`
//! (`sys::is_elevated`) and this module looks for the *High* (or *System*) mandatory integrity
//! level SID in the answer. A SID is the same in every language of Windows.
//!
//! **Fail-open, on purpose.** When `whoami` cannot be run or its answer is not understood the
//! process counts as not elevated and start-up goes on. Nothing later reliably catches an
//! elevated start: `tauri::webview_version()` only proves that a runtime can be *found* (it can
//! succeed in an elevated process), so an elevated launcher that slipped through may have its
//! `WebView2` ignore the `WEBVIEW2_*` variables and write its profile to the PC. The check is a
//! courtesy with a clear message, not a security boundary.

use crate::fatal::Fatal;

/// `S-1-16-12288`: High mandatory level, what an elevated token carries.
const HIGH_INTEGRITY: &str = "S-1-16-12288";
/// `S-1-16-16384`: System mandatory level.
const SYSTEM_INTEGRITY: &str = "S-1-16-16384";

/// `true` when `whoami /groups` output lists a High or System mandatory level.
pub fn lists_elevated_integrity(output: &str) -> bool {
    output.contains(HIGH_INTEGRITY) || output.contains(SYSTEM_INTEGRITY)
}

/// Refuses an elevated process with the message the user sees.
///
/// # Errors
///
/// [`Fatal::Elevated`] when `elevated`.
pub fn refuse_if_elevated(elevated: bool) -> Result<(), Fatal> {
    if elevated {
        Err(Fatal::Elevated)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fatal::message;

    const NORMAL: &str = "Mandatory Label\\Medium Mandatory Level   Label  S-1-16-8192   Mandatory group\n\
                          BUILTIN\\Users  Alias  S-1-5-32-545  Mandatory group";
    const ELEVATED: &str = "Mandatory Label\\High Mandatory Level   Label  S-1-16-12288  Mandatory group\n\
                            BUILTIN\\Administrators  Alias  S-1-5-32-544  Mandatory group";

    #[test]
    fn elevated_process_is_refused_with_message() {
        assert!(!lists_elevated_integrity(NORMAL));
        assert!(lists_elevated_integrity(ELEVATED));
        assert!(lists_elevated_integrity("... S-1-16-16384 ..."));
        assert_eq!(refuse_if_elevated(false), Ok(()));
        let refused = refuse_if_elevated(true);
        assert_eq!(refused, Err(Fatal::Elevated));
        let text = refused.err().map(|f| message(&f).body).unwrap_or_default();
        assert!(text.contains("administrator"), "{text}");
        assert!(text.contains("SLATECORE LAUNCHER"), "{text}");
    }
}

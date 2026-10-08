//! The launcher refuses to run elevated: `WebView2` ignores `WEBVIEW2_*` variables (the data
//! folder and the bundled runtime) in an elevated process, which would put its files on the PC.
//!
//! There is no safe std or Tauri API for the question "is this process elevated", and `unsafe`
//! is not allowed outside `env_bootstrap`. So the shell asks the system's own `whoami /groups`
//! and looks for the *High* (or *System*) mandatory integrity level SID. A SID is the same in
//! every language of Windows. The program is started by its full path under `SystemRoot`, never
//! found through `PATH` or the current folder (the drive the launcher lives on is writable by
//! anyone).

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

/// Whether this process runs elevated. Any failure to find out counts as "not elevated": the
/// check is a courtesy, `WebView2`'s own failure is caught later (`Fatal::RuntimeFailed`).
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    /// Do not flash a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Some(root) = std::env::var_os("SystemRoot") else {
        return false;
    };
    let whoami = std::path::Path::new(&root)
        .join("System32")
        .join("whoami.exe");
    match Command::new(whoami)
        .arg("/groups")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(output) => lists_elevated_integrity(&String::from_utf8_lossy(&output.stdout)),
        Err(error) => {
            log::debug!("could not check elevation: {error}");
            false
        }
    }
}

/// Only Windows ships; this keeps the crate building on other hosts.
#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
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

//! The two Windows system calls of the shell that have no home in the launcher crates: asking
//! whether the process is elevated, and starting File Explorer.
//!
//! The whole shell is Windows-only (`rust-standards.md`: no `cfg` branches for other systems), so
//! this file uses `std::os::windows` directly and has no stand-in for other hosts. The decisions
//! around these calls (what the elevation answer means, how Explorer's argument is built and what
//! may be opened) are pure and live in `elevation` and `os_open`, which are tested on any host.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::elevation::lists_elevated_integrity;
use crate::env_bootstrap::webview2_variables;

/// `CREATE_NO_WINDOW`: a console program started by the shell never flashes a window.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `%SystemRoot%`, from the environment Windows gave this process.
fn system_root() -> Option<PathBuf> {
    std::env::var_os("SystemRoot").map(PathBuf::from)
}

/// Whether this process runs elevated, by asking the system's own `whoami /groups` for the
/// mandatory integrity level (there is no safe std or Tauri API, and `unsafe` is confined to
/// `env_bootstrap`). The program is started by its full path under `SystemRoot`, never found
/// through `PATH` or the current folder.
///
/// Any failure to find out counts as "not elevated" (fail-open, on purpose): the check is a
/// courtesy that gives a clear message. It is not a security boundary, and `WebView2` failing to
/// start in an elevated process is *not* reliably caught afterwards (see `lib.rs`).
pub fn is_elevated() -> bool {
    let Some(root) = system_root() else {
        return false;
    };
    let whoami = root.join("System32").join("whoami.exe");
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

/// Starts `%SystemRoot%\explorer.exe` with `argument` as its whole command line after the program
/// name (already quoted, see `os_open::explorer_argument`). The launcher's `WEBVIEW2_*` variables
/// are removed from the child's environment: a program that opens the item may embed `WebView2`
/// and must not share (and lock) the launcher's data folder. Explorer is not awaited: it reports
/// failure through its exit code even when it worked.
pub fn spawn_explorer(argument: &OsStr) -> io::Result<()> {
    let explorer = system_root()
        .map(|root| root.join("explorer.exe"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "SystemRoot is not set"))?;
    let mut command = Command::new(explorer);
    // `raw_arg`: the argument is quoted by `explorer_argument`; `arg` would not quote a name
    // that has a comma but no space, and Explorer reads a comma as a separator.
    command
        .raw_arg(argument)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // `vars_os`: a variable with a non-Unicode name must not make the launcher panic.
    let names: Vec<OsString> = std::env::vars_os().map(|(name, _)| name).collect();
    for name in webview2_variables(names) {
        command.env_remove(name);
    }
    command.spawn().map(drop)
}

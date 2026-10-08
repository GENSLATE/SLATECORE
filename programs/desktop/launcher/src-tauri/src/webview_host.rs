//! Finds the `WebView2` runtime the PC has installed, by reading the registry. Read-only: the
//! keys are opened with `KEY_READ` and nothing is ever written.
//!
//! The Evergreen runtime registers its version (`pv`, a string) under the `EdgeUpdate` client key
//! with the `WebView2` runtime's GUID. A key can outlive a broken install, so the shell also
//! asks the real `WebView2` loader (`tauri::webview_version`) once the environment is final.
//!
//! The key paths and the `pv` value name below come from Microsoft's distribution documentation
//! as remembered when this was written (`research/versions.md` section 2, item 11). They have
//! not been confirmed on a Windows PC. Confirm them with `reg query` on a machine with
//! `WebView2` installed before relying on this; the loader check is the safety net meanwhile.

use crate::runtime::Version;

/// The `WebView2` runtime's client GUID under `EdgeUpdate\Clients`.
#[cfg(windows)]
const CLIENT: &str = r"Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

/// The newest installed PC runtime (per machine on 64-bit Windows, or per user), if any.
#[cfg(windows)]
pub fn installed_version() -> Option<Version> {
    use crate::runtime::newest_installed;
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};

    let keys = [
        (
            HKEY_LOCAL_MACHINE,
            format!(r"SOFTWARE\WOW6432Node\{CLIENT}"),
        ),
        (HKEY_CURRENT_USER, format!(r"Software\{CLIENT}")),
    ];
    let values = keys.into_iter().filter_map(|(hive, path)| {
        RegKey::predef(hive)
            .open_subkey_with_flags(path, KEY_READ)
            .ok()?
            .get_value::<String, _>("pv")
            .ok()
    });
    newest_installed(values)
}

/// Only Windows ships; this keeps the crate building on other hosts.
#[cfg(not(windows))]
pub fn installed_version() -> Option<Version> {
    None
}

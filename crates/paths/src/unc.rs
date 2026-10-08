//! Telling network (UNC) paths apart without depending on the host OS.
//!
//! The launcher only ever runs on Windows, but the checks work on the path text so the tests
//! can run anywhere. `std::path::Component::Prefix` would not help here: it only exists on
//! Windows.

use std::path::Path;

/// `true` for `\\server\share\…`, `//server/share/…` and the verbatim `\\?\UNC\server\share\…`.
///
/// Verbatim and device paths to a local volume (`\\?\C:\…`, `\\.\C:\…`) are not UNC. Windows
/// treats `/` like `\`, so both are accepted.
pub(crate) fn is_unc(path: &Path) -> bool {
    let text = path.to_string_lossy();
    // The longest prefix that matters is `\\?\UNC\` (8 characters).
    let head: String = text
        .chars()
        .take(8)
        .map(|c| if c == '/' { '\\' } else { c })
        .collect();
    if let Some(rest) = head
        .strip_prefix(r"\\?\")
        .or_else(|| head.strip_prefix(r"\\.\"))
    {
        return rest
            .get(..4)
            .is_some_and(|name| name.eq_ignore_ascii_case(r"UNC\"));
    }
    head.starts_with(r"\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn unc(path: &str) -> bool {
        is_unc(Path::new(path))
    }

    #[test]
    fn server_and_share_paths_are_unc() {
        assert!(unc(r"\\fileserver\share\SLATECORE"));
        assert!(unc(r"\\192.168.1.5\usb"));
        assert!(unc(r"\\fileserver"));
    }

    #[test]
    fn forward_slashes_are_unc_too() {
        assert!(unc("//fileserver/share/SLATECORE"));
        assert!(unc(r"\/fileserver\share"));
    }

    #[test]
    fn verbatim_unc_paths_are_unc() {
        assert!(unc(r"\\?\UNC\fileserver\share\SLATECORE"));
        assert!(unc(r"\\?\unc\fileserver\share"));
        assert!(unc("//?/UNC/fileserver/share"));
    }

    #[test]
    fn local_drive_paths_are_not_unc() {
        assert!(!unc(r"C:\SLATECORE"));
        assert!(!unc(r"S:\a b\ünï"));
        assert!(!unc("E:/portable"));
    }

    #[test]
    fn verbatim_and_device_paths_to_local_volumes_are_not_unc() {
        assert!(!unc(r"\\?\C:\SLATECORE"));
        assert!(!unc(
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\SLATECORE"
        ));
        assert!(!unc(r"\\.\C:\SLATECORE"));
    }

    #[test]
    fn relative_and_posix_paths_are_not_unc() {
        assert!(!unc(""));
        assert!(!unc("programs/genslate"));
        assert!(!unc("/home/user/SLATECORE"));
        assert!(!unc(r"\single\leading\backslash"));
    }
}

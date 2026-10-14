//! Name checks shared by app names and profile names.
//!
//! Both end up as a single folder name under the install folder, so neither may contain a
//! separator, a drive prefix, a parent reference or anything Windows treats specially. The
//! checks look at the text only and behave the same on every OS, so tests can run anywhere.

use std::path::{Component, Path};

/// Characters Windows does not allow in a file name (separators and the drive colon included).
const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// `true` for the names Windows reserves for devices: `CON`, `PRN`, `AUX`, `NUL`,
/// `COM1`-`COM9`, `LPT1`-`LPT9` (also with the superscript digits), in any case and also with an
/// extension (`con.txt`).
pub(crate) fn is_device_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end_matches(' ')
        .to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || numbered_device(&stem, "com")
        || numbered_device(&stem, "lpt")
}

/// `com1`..`com9` and `com\u{b9}`..`com\u{b3}`: the prefix and exactly one digit.
fn numbered_device(stem: &str, prefix: &str) -> bool {
    stem.strip_prefix(prefix).is_some_and(|digit| {
        let mut chars = digit.chars();
        matches!(
            (chars.next(), chars.next()),
            (Some('1'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}'), None)
        )
    })
}

/// `true` when `name` is one ordinary folder name: not empty, not `.` or `..`, no separator,
/// drive colon, wildcard, control character, trailing dot or space, and not a device name.
pub(crate) fn is_plain_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    let one_component = matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(part)), None) if part == name
    );
    one_component
        && !name.contains(FORBIDDEN)
        && !name.chars().any(char::is_control)
        && !name.ends_with(['.', ' '])
        && !is_device_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_names_are_found_in_any_case_and_with_extensions() {
        for name in [
            "con",
            "CON",
            "Prn",
            "aux",
            "NUL",
            "com1",
            "COM9",
            "lpt1",
            "LPT9",
            "con.txt",
            "nul.tar.gz",
            "COM1.log",
            "aux.",
            "com\u{b9}",
            "LPT\u{b2}.x",
            "com\u{b3}",
        ] {
            assert!(is_device_name(name), "{name:?}");
        }
    }

    #[test]
    fn ordinary_names_are_not_devices() {
        for name in [
            "console", "com", "com0", "com10", "lpt", "lpt0", "communal", "shared", "alice",
            "a.con", "recon", "",
        ] {
            assert!(!is_device_name(name), "{name:?}");
        }
    }

    #[test]
    fn plain_names_accept_ordinary_folder_names() {
        for name in [
            "shared",
            "alice",
            "Bob Smith",
            "kids-2",
            "\u{fc}n\u{ef}",
            "\u{65e5}\u{672c}\u{8a9e}",
            "a.b",
            "_x",
            "_invalid",
        ] {
            assert!(is_plain_name(name), "{name:?}");
        }
    }

    #[test]
    fn plain_names_reject_anything_but_one_ordinary_component() {
        for name in [
            "",
            ".",
            "..",
            "../x",
            r"..\x",
            "/etc",
            r"\etc",
            r"C:\x",
            "C:x",
            "C:",
            "a/b",
            r"a\b",
            "a/",
            "x:y",
            "trailing.",
            "trailing ",
            "con",
            "NUL.txt",
            "a*b",
            "a?b",
            "a<b",
            "a>b",
            "a|b",
            "a\"b",
            "a\0b",
            "a\nb",
            r"\\srv\share",
            "//srv/share",
        ] {
            assert!(!is_plain_name(name), "{name:?}");
        }
    }
}

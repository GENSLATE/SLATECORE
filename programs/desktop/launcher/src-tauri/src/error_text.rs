//! Error wording for the webview: the launcher core's errors with every path cut down to its
//! file name. A path is where the drive letter, the user's name and the folder layout leak, so
//! neither the UI nor the log gets one. No Tauri types, so it is tested on any host.

use std::path::Path;

use genslate_launcher_core::LauncherError;

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// [`LauncherError`]'s text with every path cut down to its file name.
pub fn launcher_message(error: &LauncherError) -> String {
    match error {
        LauncherError::Read { path, source } => {
            format!("could not read {}: {source}", file_name(path))
        }
        LauncherError::Write { path, source } => {
            format!("could not write {}: {source}", file_name(path))
        }
        LauncherError::Parse { path, message } => {
            format!("{} is invalid: {message}", file_name(path))
        }
        LauncherError::OutsideRoot(path) => {
            format!(
                "{} is not inside an allowed programs folder",
                file_name(path)
            )
        }
        LauncherError::Spawn { path, source } => {
            format!("could not start {}: {source}", file_name(path))
        }
        other => other.to_string(),
    }
}

/// `text` with every file path replaced by `<path>`: a drive path (`C:\..`, `c:/..`), a UNC path
/// (`\\server\..`) or a rooted path (`/home/..`) that starts a word. A path may hold spaces, so
/// it runs to the end of the text, or to the closing quote or bracket when it was opened by one.
/// For logging the text of an error that may name a path (Tauri's, an OS's).
pub fn without_paths(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let at_boundary = i == 0
            || chars[i - 1].is_whitespace()
            || matches!(chars[i - 1], '"' | '\'' | '`' | '(' | '[' | '=');
        if at_boundary && starts_path(&chars[i..]) {
            out.push_str("<path>");
            let closer = (i > 0)
                .then(|| match chars[i - 1] {
                    '"' => Some('"'),
                    '\'' => Some('\''),
                    '`' => Some('`'),
                    '(' => Some(')'),
                    '[' => Some(']'),
                    _ => None,
                })
                .flatten();
            i = match closer {
                Some(close) => chars[i..]
                    .iter()
                    .position(|c| *c == close)
                    .map_or(chars.len(), |offset| i + offset),
                None => chars.len(),
            };
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Whether `rest` begins like a path: `X:\` or `X:/`, `\\`, or `/` + a name character.
fn starts_path(rest: &[char]) -> bool {
    match rest {
        [drive, ':', '\\' | '/', ..] if drive.is_ascii_alphabetic() => true,
        ['\\', '\\', ..] => true,
        ['/', next, ..] => next.is_alphanumeric() || matches!(next, '.' | '_' | '~'),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::path::PathBuf;

    #[test]
    fn paths_of_every_spelling_are_stripped_from_free_text() {
        let cases = [
            (r"cannot open C:\Users\Jane Doe\x.dll", "cannot open <path>"),
            ("cannot open c:/Users/jane/x.dll", "cannot open <path>"),
            (r"share \\server\share\dir\f.txt is gone", "share <path>"),
            ("failed at /home/jane/.local/app.db", "failed at <path>"),
            (
                r#"error: "C:\Program Files\App\a.exe" is missing"#,
                r#"error: "<path>" is missing"#,
            ),
            (r"(D:\data\x) failed", "(<path>) failed"),
            ("code=/home/jane/x.db now", "code=<path>"),
        ];
        for (input, want) in cases {
            assert_eq!(without_paths(input), want, "{input}");
        }
    }

    #[test]
    fn text_without_a_path_is_unchanged() {
        for text in [
            "window `main` does not exist",
            "read/write access and/or sharing: denied",
            "version 1.2.3 at 12:30",
            "a: b",
            "",
        ] {
            assert_eq!(without_paths(text), text);
        }
    }

    #[test]
    fn messages_keep_file_names_and_drop_folders() {
        let text = launcher_message(&LauncherError::Read {
            path: PathBuf::from("/drive/SLATECORE/other/launcher/configs/settings.toml"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "access denied"),
        });
        assert!(text.contains("settings.toml"), "{text}");
        assert!(!text.contains("SLATECORE"), "{text}");
        assert!(!text.contains("/drive"), "{text}");
    }

    #[test]
    fn spawn_and_outside_root_errors_name_only_the_program() {
        let spawn = launcher_message(&LauncherError::Spawn {
            path: PathBuf::from("/drive/x/programs/genslate/explorer/explorer.exe"),
            source: io::Error::other("blocked"),
        });
        assert!(
            spawn.contains("explorer.exe") && !spawn.contains("programs"),
            "{spawn}"
        );
        let outside = launcher_message(&LauncherError::OutsideRoot(PathBuf::from(
            "/drive/Windows/x.exe",
        )));
        assert!(outside.starts_with("x.exe"), "{outside}");
    }

    #[test]
    fn errors_without_paths_pass_through() {
        let text = launcher_message(&LauncherError::UnknownApp("zzz".to_owned()));
        assert_eq!(
            text,
            LauncherError::UnknownApp("zzz".to_owned()).to_string()
        );
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::path::PathBuf;

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

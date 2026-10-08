//! The folders and files the UI can ask the shell to open, by name. The UI never sends a path:
//! a name picks one of these, and the path is built here from `genslate-paths`.

use std::path::{Path, PathBuf};

use genslate_launcher_core::config::{KEYBINDINGS_FILE, SETTINGS_FILE};
use serde::Deserialize;

/// A folder of the portable storage (`storage/users/shared/<Name>`), or the storage root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SharedFolder {
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
    Storage,
}

impl SharedFolder {
    /// Every folder, in the order of the command registry's `/folder` choices.
    pub const ALL: [Self; 7] = [
        Self::Desktop,
        Self::Documents,
        Self::Downloads,
        Self::Music,
        Self::Pictures,
        Self::Videos,
        Self::Storage,
    ];

    /// The lower-case name the UI and the slash command use.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Documents => "documents",
            Self::Downloads => "downloads",
            Self::Music => "music",
            Self::Pictures => "pictures",
            Self::Videos => "videos",
            Self::Storage => "storage",
        }
    }

    /// Parses the slash-command spelling (`/folder documents`), in any letter case.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|folder| folder.id().eq_ignore_ascii_case(name.trim()))
    }

    /// The folder on disk: inside the profile folder, or the storage root itself.
    pub fn path(self, profile: &Path, storage: &Path) -> PathBuf {
        let name = match self {
            Self::Desktop => "Desktop",
            Self::Documents => "Documents",
            Self::Downloads => "Downloads",
            Self::Music => "Music",
            Self::Pictures => "Pictures",
            Self::Videos => "Videos",
            Self::Storage => return storage.to_path_buf(),
        };
        profile.join(name)
    }
}

/// The launcher's own files and folders the user can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigFile {
    Settings,
    Keybindings,
    Logs,
}

impl ConfigFile {
    /// The file name inside `configs/`, or `None` for the log folder.
    pub const fn file_name(self) -> Option<&'static str> {
        match self {
            Self::Settings => Some(SETTINGS_FILE),
            Self::Keybindings => Some(KEYBINDINGS_FILE),
            Self::Logs => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_launcher_core::actions::FOLDERS;

    #[test]
    fn parses_folder_names_in_any_case_and_nothing_else() {
        assert_eq!(
            SharedFolder::parse("Documents"),
            Some(SharedFolder::Documents)
        );
        assert_eq!(
            SharedFolder::parse(" storage "),
            Some(SharedFolder::Storage)
        );
        assert_eq!(SharedFolder::parse("../etc"), None);
        assert_eq!(SharedFolder::parse(""), None);
    }

    #[test]
    fn the_folders_match_the_registry_choices() {
        let ids: Vec<&str> = SharedFolder::ALL.iter().map(|folder| folder.id()).collect();
        assert_eq!(ids, FOLDERS);
    }

    #[test]
    fn folders_map_into_the_profile_and_storage_is_the_root() {
        let profile = Path::new("/suite/storage/users/shared");
        let storage = Path::new("/suite/storage");
        assert_eq!(
            SharedFolder::Music.path(profile, storage),
            profile.join("Music")
        );
        assert_eq!(SharedFolder::Storage.path(profile, storage), storage);
    }

    #[test]
    fn the_frontend_spellings_deserialize() -> Result<(), serde_json::Error> {
        assert_eq!(
            serde_json::from_str::<SharedFolder>("\"pictures\"")?,
            SharedFolder::Pictures
        );
        assert_eq!(
            serde_json::from_str::<ConfigFile>("\"keybindings\"")?,
            ConfigFile::Keybindings
        );
        assert!(serde_json::from_str::<SharedFolder>("\"C:\\\\\"").is_err());
        Ok(())
    }

    #[test]
    fn the_config_files_are_the_cores_names() {
        assert_eq!(ConfigFile::Settings.file_name(), Some("settings.toml"));
        assert_eq!(
            ConfigFile::Keybindings.file_name(),
            Some("keybindings.toml")
        );
        assert_eq!(ConfigFile::Logs.file_name(), None);
    }
}

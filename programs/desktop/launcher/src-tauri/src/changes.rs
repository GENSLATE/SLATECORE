//! Which part of the launcher a burst of file changes concerns. Pure, so it is tested on any host;
//! `reload` feeds it what the file watcher reports.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use genslate_launcher_core::config::{KEYBINDINGS_FILE, SETTINGS_FILE};
use genslate_launcher_core::metadata::metadata_dir;
use genslate_paths::AppPaths;

/// What has to be reloaded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Changes {
    /// `settings.toml` or `keybindings.toml` was edited.
    pub settings: bool,
    /// An app was added, removed or changed, or the tab settings were edited.
    pub catalog: bool,
}

fn is_named(path: &Path, name: &str) -> bool {
    path.file_name()
        .is_some_and(|file| file.to_string_lossy().eq_ignore_ascii_case(name))
}

/// Sorts `changed` into the parts of the launcher it touches. `is_own` says whether the
/// launcher itself wrote the file (those are ignored: it already applied them).
pub fn classify(
    changed: &BTreeSet<PathBuf>,
    paths: &AppPaths,
    is_own: impl Fn(&Path) -> bool,
) -> Changes {
    let metadata = metadata_dir(paths);
    let programs = paths.layout.programs.as_deref();
    let mut found = Changes::default();
    for path in changed.iter().filter(|path| !is_own(path)) {
        let in_config = path.parent() == Some(paths.config_dir.as_path());
        if in_config && (is_named(path, SETTINGS_FILE) || is_named(path, KEYBINDINGS_FILE)) {
            found.settings = true;
        }
        if path.starts_with(&metadata) || programs.is_some_and(|dir| path.starts_with(dir)) {
            found.catalog = true;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_paths::{Environment, resolve_with};
    use genslate_testing::{TempTree, fake_suite};
    use std::error::Error;

    fn paths_in(tmp: &TempTree) -> Result<AppPaths, Box<dyn Error>> {
        let root = fake_suite(tmp.path())?;
        let env = Environment {
            install_dir_override: Some(root),
            ..Environment::default()
        };
        Ok(resolve_with("launcher", &env)?)
    }

    fn set(paths: impl IntoIterator<Item = PathBuf>) -> BTreeSet<PathBuf> {
        paths.into_iter().collect()
    }

    #[test]
    fn editing_the_settings_files_reloads_settings_only() -> Result<(), Box<dyn Error>> {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let edited = set([paths.config_dir.join("Settings.TOML")]);
        assert_eq!(
            classify(&edited, &paths, |_| false),
            Changes {
                settings: true,
                catalog: false
            }
        );
        let keys = set([paths.config_dir.join("keybindings.toml")]);
        assert!(classify(&keys, &paths, |_| false).settings);
        Ok(())
    }

    #[test]
    fn the_atomic_write_temp_file_is_not_a_settings_edit() -> Result<(), Box<dyn Error>> {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let half_written = set([paths.config_dir.join("settings.toml.tmp")]);
        assert_eq!(
            classify(&half_written, &paths, |_| false),
            Changes::default()
        );
        Ok(())
    }

    #[test]
    fn programs_and_tab_settings_rescan_the_catalog() -> Result<(), Box<dyn Error>> {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let programs = paths.layout.programs.clone().ok_or("no programs folder")?;
        let installed = set([programs
            .join("portapps.io")
            .join("firefox")
            .join("firefox.exe")]);
        assert_eq!(
            classify(&installed, &paths, |_| false),
            Changes {
                settings: false,
                catalog: true
            }
        );
        let tabs = set([metadata_dir(&paths).join("genslate.toml")]);
        assert!(classify(&tabs, &paths, |_| false).catalog);
        Ok(())
    }

    #[test]
    fn the_launchers_own_writes_and_unrelated_files_are_ignored() -> Result<(), Box<dyn Error>> {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let own = metadata_dir(&paths).join("genslate.toml");
        let changed = set([own.clone(), paths.log_dir.join("launcher.log")]);
        assert_eq!(
            classify(&changed, &paths, |path| path == own),
            Changes::default()
        );
        Ok(())
    }
}

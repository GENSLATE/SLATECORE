//! Recently launched apps, kept in `other/launcher/database/recent.toml` (data, not settings,
//! so editing the config never races with launch bookkeeping). The file holds app ids and
//! Unix seconds, never a path.

use std::path::{Path, PathBuf};

use genslate_paths::AppPaths;
use serde::{Deserialize, Serialize};

use crate::LauncherError;
use crate::catalog::AppId;
use crate::config::load_toml;
use crate::metadata::write_atomic;

/// File name of the launch history inside `other/launcher/database/`.
pub const RECENT_FILE: &str = "recent.toml";

/// `other/launcher/database/recent.toml`.
pub fn recent_path(paths: &AppPaths) -> PathBuf {
    paths.database_dir.join(RECENT_FILE)
}

/// How many launches are remembered.
const CAPACITY: usize = 50;

/// The launch history, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentLaunches {
    launch: Vec<Launch>,
}

/// One launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launch {
    pub id: AppId,
    /// Unix seconds.
    pub at: u64,
}

impl RecentLaunches {
    /// Reads the history; a missing or broken file is an empty history.
    pub fn load(path: &Path) -> Self {
        load_toml(path).unwrap_or_else(|error: LauncherError| {
            log::warn!("{error}; starting a new launch history");
            Self::default()
        })
    }

    /// Records a launch and saves the history.
    pub fn record(&mut self, id: AppId, at: u64, path: &Path) -> Result<(), LauncherError> {
        self.launch.retain(|launch| launch.id != id);
        self.launch.insert(0, Launch { id, at });
        self.launch.truncate(CAPACITY);
        let text = toml::to_string(self).map_err(|error| LauncherError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        write_atomic(path, &text)
    }

    /// The `limit` most recent distinct apps.
    pub fn latest(&self, limit: usize) -> Vec<AppId> {
        self.launch
            .iter()
            .take(limit)
            .map(|launch| launch.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Source;
    use genslate_paths::{Environment, resolve_with};
    use genslate_testing::{TempTree, fake_suite};
    use std::fs;

    #[test]
    fn keeps_distinct_ids_newest_first_and_persists() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?;
        let path = tree.join("launcher/recent.toml");
        let explorer = AppId::new(Source::Genslate, "explorer");
        let firefox = AppId::new(Source::PortableApps, "FirefoxPortable");
        let mut recent = RecentLaunches::load(&path);
        recent.record(explorer.clone(), 1, &path)?;
        recent.record(firefox.clone(), 2, &path)?;
        recent.record(explorer.clone(), 3, &path)?;
        assert_eq!(recent.latest(5), [explorer.clone(), firefox.clone()]);
        assert_eq!(RecentLaunches::load(&path).latest(1), [explorer]);
        Ok(())
    }

    #[test]
    fn broken_history_starts_over() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?.file("recent.toml", "launch = 5")?;
        assert_eq!(
            RecentLaunches::load(&tree.join("recent.toml")).latest(5),
            []
        );
        Ok(())
    }

    #[test]
    fn recent_is_stored_under_other_launcher_database() -> Result<(), Box<dyn std::error::Error>> {
        let tree = TempTree::new()?;
        let root = fake_suite(tree.path())?;
        let env = Environment {
            exe: Some(root.join("programs/genslate/launcher/slatecore-launcher.exe")),
            ..Environment::default()
        };
        let paths = resolve_with("launcher", &env)?;
        paths.create_dirs()?;
        let path = recent_path(&paths);
        assert_eq!(path, root.join("other/launcher/database/recent.toml"));

        let mut recent = RecentLaunches::load(&path);
        recent.record(AppId::new(Source::Genslate, "explorer"), 7, &path)?;
        assert!(path.is_file());
        assert_eq!(
            RecentLaunches::load(&path).latest(1),
            [AppId::new(Source::Genslate, "explorer")]
        );
        // Not a setting (configs/) and not disposable (cache/): nothing else was written.
        for other in ["configs", "cache", "logs"] {
            assert!(
                fs::read_dir(root.join("other/launcher").join(other))?
                    .filter_map(Result::ok)
                    .all(|entry| entry.file_name() != "recent.toml"),
                "recent.toml leaked into {other}"
            );
        }
        // The launch time is data; the file holds ids and seconds, never a path.
        let text = fs::read_to_string(&path)?;
        assert!(text.contains("genslate/explorer"), "{text}");
        assert!(!text.contains(root.to_string_lossy().as_ref()), "{text}");
        Ok(())
    }
}

//! Opening folders and files for the UI: portable storage folders go to GENSLATE Explorer when
//! it is installed and to File Explorer otherwise; the settings files open in the default
//! editor. Every path is built here from an id or a name, never taken from the webview.

use std::fs;
use std::path::Path;

use genslate_launcher_core::LauncherError;
use genslate_launcher_core::catalog::{AppId, Source};
use genslate_launcher_core::launch::LaunchSpec;
use genslate_launcher_core::metadata::write_atomic;

use crate::error::AppError;
use crate::folders::{ConfigFile, SharedFolder};
use crate::os_open;
use crate::state::Launcher;

fn open_error(error: &std::io::Error) -> AppError {
    AppError::Open(error.to_string())
}

fn create_dir(path: &Path) -> Result<(), AppError> {
    fs::create_dir_all(path).map_err(|source| {
        AppError::Launcher(LauncherError::Write {
            path: path.to_path_buf(),
            source,
        })
    })
}

/// Opens a storage folder (created if it is missing).
pub fn open_shared_folder(launcher: &Launcher, folder: SharedFolder) -> Result<(), AppError> {
    let path = folder.path(&launcher.profile_dir(), &launcher.paths.layout.storage);
    create_dir(&path)?;
    open_in_file_manager(launcher, &path)
}

/// GENSLATE Explorer if it is installed and can start, else File Explorer.
pub fn open_in_file_manager(launcher: &Launcher, path: &Path) -> Result<(), AppError> {
    let explorer = launcher
        .catalog()
        .find(&AppId::new(Source::Genslate, "explorer"))
        .filter(|entry| entry.status.is_launchable())
        .cloned();
    if let Some(explorer) = explorer {
        let spec = LaunchSpec::resolve(
            &explorer,
            Some(vec![path.to_string_lossy().into_owned()]),
            &launcher.allowed_roots,
        )?;
        spec.spawn()?;
        return Ok(());
    }
    os_open::open_path(path).map_err(|error| open_error(&error))
}

/// Opens `settings.toml` or `keybindings.toml` in the default editor (created empty when it does
/// not exist yet, so there is something to edit), or the log folder.
pub fn open_config(launcher: &Launcher, file: ConfigFile) -> Result<(), AppError> {
    let Some(name) = file.file_name() else {
        create_dir(&launcher.paths.log_dir)?;
        return open_in_file_manager(launcher, &launcher.paths.log_dir);
    };
    let path = launcher.paths.config_dir.join(name);
    if !path.exists() {
        write_atomic(&path, "")?;
    }
    os_open::open_path(&path).map_err(|error| open_error(&error))
}

/// Opens an app's own folder.
pub fn open_app_folder(launcher: &Launcher, id: &AppId) -> Result<(), AppError> {
    let dir = launcher
        .catalog()
        .find(id)
        .and_then(|entry| entry.dir.clone())
        .ok_or_else(|| AppError::InvalidArgument(format!("{id} has no folder")))?;
    open_in_file_manager(launcher, &dir)
}

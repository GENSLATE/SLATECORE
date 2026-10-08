//! Hot reload: the settings files and the app folders are watched and edits apply without a
//! restart. A file the launcher wrote itself is ignored, it has already been applied.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use genslate_launcher_core::catalog::Source;
use genslate_launcher_core::metadata::metadata_dir;
use genslate_launcher_core::watch::{self, WatchTarget};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::changes::classify;
use crate::state::Launcher;
use crate::{events, hotkey, window};

/// How long the watcher waits for a burst of file events to settle.
const SETTLE: Duration = Duration::from_millis(200);

/// Starts watching the config, metadata and program folders.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let launcher = app.state::<Launcher>();
    let mut targets = vec![
        WatchTarget {
            path: launcher.paths.config_dir.clone(),
            recursive: false,
        },
        WatchTarget {
            path: metadata_dir(&launcher.paths),
            recursive: false,
        },
    ];
    if let Some(programs) = &launcher.paths.layout.programs {
        targets.extend(Source::ALL.iter().map(|source| WatchTarget {
            path: programs.join(source.folder()),
            // A new app is a folder with files; one level of depth is enough to notice it.
            recursive: true,
        }));
    }
    let handle = app.clone();
    match watch::watch(&targets, SETTLE, move |changed| {
        on_change(&handle, &changed);
    }) {
        Ok(watcher) => {
            if let Ok(mut slot) = launcher.watcher.lock() {
                *slot = Some(watcher);
            }
        }
        Err(error) => log::warn!("hot reload is off: {}", error.kind()),
    }
}

fn on_change<R: Runtime>(app: &AppHandle<R>, changed: &BTreeSet<PathBuf>) {
    let launcher = app.state::<Launcher>();
    let touched = classify(changed, &launcher.paths, |path| {
        launcher.self_writes.is_own(path)
    });
    if touched.settings {
        reload_settings(app);
    }
    if touched.catalog {
        launcher.rescan();
        emit(app, events::CATALOG, ());
    }
}

/// Reads both settings files again and applies what changed: the global shortcut, the window
/// size and the pin. Tells the UI when anything it shows is different.
pub fn reload_settings<R: Runtime>(app: &AppHandle<R>) {
    let launcher = app.state::<Launcher>();
    let previous = launcher.settings().clone();
    let mut next = previous.clone();
    if !next.reload(&launcher.paths.config_dir) {
        return;
    }
    *launcher.settings_mut() = next.clone();

    if previous.keybindings.global.toggle != next.keybindings.global.toggle {
        hotkey::register(app, &next.keybindings.global.toggle);
    }
    if previous.config.appearance.size != next.config.appearance.size
        && let Err(error) = window::apply_size(app)
    {
        log::warn!("could not resize: {}", error.message());
    }
    if previous.config.behavior.pinned != next.config.behavior.pinned
        && let Err(error) = window::set_pinned(app, next.config.behavior.pinned)
    {
        log::warn!("could not pin: {}", error.message());
    }
    emit(app, events::SETTINGS, next);
}

/// Emits an event; a failed emit only means the UI is not listening yet.
pub fn emit<R: Runtime, S: Serialize + Clone>(app: &AppHandle<R>, event: &str, payload: S) {
    if let Err(error) = app.emit(event, payload) {
        log::debug!("emit {event}: {error}");
    }
}

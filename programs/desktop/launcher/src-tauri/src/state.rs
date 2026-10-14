//! [`Launcher`]: the shared state behind every command, event and background thread. It holds no
//! Tauri types, so it is built and tested on any host.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use genslate_launcher_core::catalog::{Catalog, CatalogRoots};
use genslate_launcher_core::config::Settings;
use genslate_launcher_core::geometry::Rect;
use genslate_launcher_core::recent::{RecentLaunches, recent_path};
use genslate_launcher_core::watch::{FileWatcher, SelfWrites};
use genslate_paths::{AppPaths, SHARED_PROFILE};

use crate::debounce::{RecentBlur, RecentHide};
use crate::vault_dto::VaultErrorDto;
use crate::vault_service::VaultService;

/// The vault, or why this run has none.
#[derive(Debug, Clone)]
pub enum VaultSlot {
    /// Open: commands run against it.
    Ready(Arc<VaultService>),
    /// It could not be opened (another launcher holds it, its header is damaged, the drive
    /// refuses). The launcher still runs; every vault command answers with this error.
    Unavailable(VaultErrorDto),
}

/// Where the launcher window is, as the shell last placed or measured it. The click-through
/// thread reads this about 60 times a second instead of asking the window each time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameGeometry {
    /// The window, in physical pixels.
    pub window: Rect,
    /// The scale factor of the monitor it is on.
    pub scale: f64,
}

/// Window state owned by the shell.
#[derive(Debug, Default)]
pub struct WindowFlags {
    pub pinned: bool,
    pub expanded: bool,
    pub visible: bool,
    /// When the window last hid: a tray click hides it (blur) and then toggles.
    pub recent_hide: RecentHide,
    /// When the window last lost focus: a tray click on a launcher that a blur does not hide
    /// (pinned, or hide-on-blur off) blurs it and then toggles, and means "hide".
    pub recent_blur: RecentBlur,
    pub geometry: Option<FrameGeometry>,
}

/// Everything the launcher shares across threads (managed by Tauri).
#[derive(Debug)]
pub struct Launcher {
    pub paths: AppPaths,
    pub roots: CatalogRoots,
    /// Programs may only be started from inside these folders.
    pub allowed_roots: Vec<PathBuf>,
    pub settings: RwLock<Settings>,
    pub catalog: RwLock<Catalog>,
    pub recent: Mutex<RecentLaunches>,
    pub self_writes: SelfWrites,
    pub window: Mutex<WindowFlags>,
    /// The UI is showing a popup that may reach outside the frame: no click-through.
    pub popup_open: AtomicBool,
    /// The status bar wants telemetry samples.
    pub telemetry_active: AtomicBool,
    pub watcher: Mutex<Option<FileWatcher>>,
    pub vault: VaultSlot,
}

impl Launcher {
    /// Builds the state for `paths`: reads both settings files, scans the catalog once.
    pub fn new(paths: AppPaths, vault: VaultSlot) -> Self {
        let settings = Settings::load(&paths.config_dir);
        let roots = CatalogRoots::from_paths(&paths);
        let pinned = settings.config.behavior.pinned;
        Self {
            allowed_roots: roots.launch_roots(),
            catalog: RwLock::new(Catalog::scan(&roots)),
            recent: Mutex::new(RecentLaunches::load(&recent_path(&paths))),
            roots,
            settings: RwLock::new(settings),
            self_writes: SelfWrites::default(),
            window: Mutex::new(WindowFlags {
                pinned,
                ..WindowFlags::default()
            }),
            popup_open: AtomicBool::new(false),
            telemetry_active: AtomicBool::new(false),
            watcher: Mutex::new(None),
            vault,
            paths,
        }
    }

    pub fn settings(&self) -> RwLockReadGuard<'_, Settings> {
        self.settings.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn settings_mut(&self) -> RwLockWriteGuard<'_, Settings> {
        self.settings
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn catalog(&self) -> RwLockReadGuard<'_, Catalog> {
        self.catalog.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn catalog_mut(&self) -> RwLockWriteGuard<'_, Catalog> {
        self.catalog.write().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn recent(&self) -> MutexGuard<'_, RecentLaunches> {
        self.recent.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn window(&self) -> MutexGuard<'_, WindowFlags> {
        self.window.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Rescans every app source.
    pub fn rescan(&self) {
        let fresh = Catalog::scan(&self.roots);
        *self.catalog_mut() = fresh;
    }

    /// `storage/users/shared`: the folders `/folder` opens live here.
    pub fn profile_dir(&self) -> PathBuf {
        self.paths.profile_dir(SHARED_PROFILE)
    }

    pub fn is_visible(&self) -> bool {
        self.window().visible
    }

    pub fn popup_open(&self) -> bool {
        self.popup_open.load(Ordering::Relaxed)
    }

    /// The vault service, if this run has one.
    pub fn vault_service(&self) -> Option<&Arc<VaultService>> {
        match &self.vault {
            VaultSlot::Ready(service) => Some(service),
            VaultSlot::Unavailable(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_paths::{Environment, resolve_with};
    use genslate_testing::{TempTree, fake_suite};
    use std::error::Error;

    #[test]
    fn the_state_is_built_from_the_install_folder_alone() -> Result<(), Box<dyn Error>> {
        let tmp = TempTree::new()?;
        let root = fake_suite(tmp.path())?;
        let env = Environment {
            install_dir_override: Some(root.clone()),
            ..Environment::default()
        };
        let paths = resolve_with("launcher", &env)?;
        paths.create_dirs()?;
        let (service, _) = VaultService::open(&paths)?;

        let launcher = Launcher::new(paths, VaultSlot::Ready(Arc::new(service)));

        assert!(!launcher.is_visible());
        assert!(!launcher.window().pinned, "defaults: not pinned");
        assert!(
            launcher
                .allowed_roots
                .iter()
                .all(|launch_root| launch_root.starts_with(&root)),
            "programs may only start from inside the install folder"
        );
        assert!(launcher.profile_dir().starts_with(&root));
        assert!(launcher.vault_service().is_some());
        Ok(())
    }

    #[test]
    fn an_unavailable_vault_has_no_service_but_keeps_its_reason() {
        let slot = VaultSlot::Unavailable(VaultErrorDto::from(genslate_vault::VaultError::Busy));
        assert!(matches!(&slot, VaultSlot::Unavailable(error) if error.code == "BUSY"));
        assert!(!matches!(slot, VaultSlot::Ready(_)));
    }
}

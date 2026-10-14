//! IPC commands. Thin: validate the input, delegate to `genslate-launcher-core` and the shell
//! modules. The UI sends app **ids** and folder and file **names**, never paths.

use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use genslate_launcher_core::actions::{ActionSpec, REGISTRY};
use genslate_launcher_core::catalog::{AppEntry, AppId, TabInfo};
use genslate_launcher_core::geometry::{EXPANDED_FRAME_WIDTH, FRAME_INSET, NORMAL_FRAME_WIDTH};
use genslate_launcher_core::launch::LaunchSpec;
use genslate_launcher_core::metadata::{self, OverridePatch, metadata_dir};
use genslate_launcher_core::recent::recent_path;
use genslate_launcher_core::system::{self, VolumeInfo};
use genslate_launcher_core::{LauncherError, config::Settings};
use genslate_paths::Mode;
use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::error::AppError;
use crate::folders::{ConfigFile, SharedFolder};
use crate::launch_args;
use crate::reload::{emit, reload_settings};
use crate::settings_write::{self, SettingKey};
use crate::state::Launcher;
use crate::window::ShowView;
use crate::{events, files, tray_menu, window};

/// Everything the UI needs at start.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherContext {
    pub version: String,
    /// `suite` (inside an install folder) or `dev` (a checkout).
    pub mode: &'static str,
    /// The install folder's name (shown under the profile).
    pub suite_name: Option<String>,
    pub profile: &'static str,
    pub settings: Settings,
    pub pinned: bool,
    pub actions: &'static [ActionSpec],
    pub layout: FrameLayout,
}

/// Frame geometry shared with the CSS, so hit testing and drawing agree.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameLayout {
    pub inset: f64,
    pub normal_width: f64,
    pub expanded_width: f64,
}

#[tauri::command]
pub fn get_context(app: AppHandle, launcher: State<'_, Launcher>) -> LauncherContext {
    let layout = &launcher.paths.layout;
    LauncherContext {
        version: app.package_info().version.to_string(),
        mode: match layout.mode {
            Mode::Suite => "suite",
            Mode::Dev => "dev",
        },
        suite_name: layout.name().map(str::to_owned),
        profile: "Shared",
        settings: launcher.settings().clone(),
        pinned: launcher.window().pinned,
        actions: REGISTRY,
        layout: FrameLayout {
            inset: FRAME_INSET,
            normal_width: NORMAL_FRAME_WIDTH,
            expanded_width: EXPANDED_FRAME_WIDTH,
        },
    }
}

/// The catalog as the UI shows it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppList {
    pub apps: Vec<AppEntry>,
    pub tabs: Vec<TabInfo>,
    /// Recently launched ids, newest first.
    pub recent: Vec<AppId>,
}

/// How many recent launches the UI gets.
const RECENT_SHOWN: usize = 8;

#[tauri::command]
pub fn list_apps(launcher: State<'_, Launcher>) -> AppList {
    let catalog = launcher.catalog();
    AppList {
        apps: catalog.apps().to_vec(),
        tabs: catalog.tabs(),
        recent: launcher.recent().latest(RECENT_SHOWN),
    }
}

#[tauri::command]
pub fn rescan(app: AppHandle, launcher: State<'_, Launcher>) {
    launcher.rescan();
    emit(&app, events::CATALOG, ());
}

/// Starts an app. `args` replaces the app's configured arguments and is accepted only from the
/// launcher window (`main`), never from the tray menu window, and is capped in count and length
/// (`launch_args`): an argument list is a command line, so a page that is not the launcher's own
/// must not choose one.
#[tauri::command]
pub fn launch_app(
    window: WebviewWindow,
    id: String,
    args: Option<Vec<String>>,
) -> Result<(), AppError> {
    launch_args::check_launch(window.label(), args.as_deref())
        .map_err(|error| AppError::InvalidArgument(error.to_string()))?;
    launch(window.app_handle(), &parse_id(&id)?, args)
}

/// Starts `id`, records it as recent and hides the launcher unless it is pinned.
pub fn launch(app: &AppHandle, id: &AppId, args: Option<Vec<String>>) -> Result<(), AppError> {
    let launcher = app.state::<Launcher>();
    let entry = launcher
        .catalog()
        .find(id)
        .cloned()
        .ok_or_else(|| LauncherError::UnknownApp(id.to_string()))?;
    let spec = LaunchSpec::resolve(&entry, args, &launcher.allowed_roots)?;
    let pid = spec.spawn()?;
    log::info!("started {id} (pid {pid})");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    if let Err(error) = launcher
        .recent()
        .record(id.clone(), now, &recent_path(&launcher.paths))
    {
        log::warn!("recent launches: {}", AppError::from(error).message());
    }
    emit(app, events::CATALOG, ());
    let pinned = launcher.window().pinned;
    if !pinned && launcher.settings().config.behavior.hide_on_launch {
        window::hide(app)?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_app_folder(launcher: State<'_, Launcher>, id: String) -> Result<(), AppError> {
    files::open_app_folder(&launcher, &parse_id(&id)?)
}

#[tauri::command]
pub fn set_app_override(app: AppHandle, id: String, patch: OverridePatch) -> Result<(), AppError> {
    // The arguments stored for an app are what `launch_app` starts it with later.
    if let Some(Some(args)) = &patch.args {
        launch_args::check_list(args)
            .map_err(|error| AppError::InvalidArgument(error.to_string()))?;
    }
    apply_override(&app, &parse_id(&id)?, &patch)
}

/// Writes an override (comments kept), rescans and tells the UI.
pub fn apply_override(app: &AppHandle, id: &AppId, patch: &OverridePatch) -> Result<(), AppError> {
    let launcher = app.state::<Launcher>();
    let dir = metadata_dir(&launcher.paths);
    let text = metadata::write_override(&dir, id.source, &id.key, patch)?;
    launcher
        .self_writes
        .record(&dir.join(id.source.settings_file()), &text);
    launcher.rescan();
    emit(app, events::CATALOG, ());
    Ok(())
}

#[tauri::command]
pub fn open_folder(launcher: State<'_, Launcher>, folder: SharedFolder) -> Result<(), AppError> {
    files::open_shared_folder(&launcher, folder)
}

#[tauri::command]
pub fn open_config_file(launcher: State<'_, Launcher>, file: ConfigFile) -> Result<(), AppError> {
    files::open_config(&launcher, file)
}

#[tauri::command]
pub fn set_setting(app: AppHandle, key: SettingKey, value: String) -> Result<(), AppError> {
    write_setting(&app, key, &value)
}

/// Validates and writes one setting (comments kept), then applies it right away; the file
/// watcher sees the same change a moment later and finds nothing left to do.
pub fn write_setting(app: &AppHandle, key: SettingKey, value: &str) -> Result<(), AppError> {
    let launcher = app.state::<Launcher>();
    let file = launcher
        .paths
        .config_dir
        .join(genslate_launcher_core::config::SETTINGS_FILE);
    settings_write::write_setting(&file, key, value)?;
    reload_settings(app);
    Ok(())
}

#[tauri::command]
pub fn get_volume_info(launcher: State<'_, Launcher>) -> Option<VolumeInfo> {
    system::volume_for(&launcher.paths.layout.root)
}

#[tauri::command]
pub fn set_telemetry_active(launcher: State<'_, Launcher>, active: bool) {
    launcher.telemetry_active.store(active, Ordering::Relaxed);
}

#[tauri::command]
pub fn window_set_pinned(app: AppHandle, pinned: bool) -> Result<(), AppError> {
    window::set_pinned(&app, pinned)
}

#[tauri::command]
pub fn window_set_expanded(app: AppHandle, expanded: bool) {
    window::set_expanded(&app, expanded);
}

#[tauri::command]
pub fn window_set_popup_open(app: AppHandle, open: bool) {
    window::set_popup(&app, open);
}

#[tauri::command]
pub fn window_hide(app: AppHandle) -> Result<(), AppError> {
    window::hide(&app)
}

#[tauri::command]
pub fn window_show(app: AppHandle, view: Option<ShowView>) -> Result<(), AppError> {
    window::show_view(&app, view.unwrap_or_default())
}

#[tauri::command]
pub fn tray_menu_hide(app: AppHandle) -> Result<(), AppError> {
    tray_menu::hide(&app)
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

/// An app id (`genslate/explorer`) from the UI.
pub fn parse_id(id: &str) -> Result<AppId, AppError> {
    id.parse().map_err(AppError::InvalidArgument)
}

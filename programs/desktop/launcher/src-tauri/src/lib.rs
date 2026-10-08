//! SLATECORE LAUNCHER by GENSLATE: the Tauri 2 shell around the launcher UI.
//!
//! Start-up, in this order (`run`):
//!
//! 1. refuse to run elevated, resolve the install folder (`genslate-paths`, no fallback);
//! 2. choose the `WebView2` runtime (the PC's, else the one bundled in installDir) and point
//!    `TEMP`, `TMP` and the `WebView2` variables into installDir (`env_bootstrap`, the one
//!    `unsafe` block of the workspace);
//! 3. build the app with the single-instance plugin first, a per-drive identity and every
//!    Tauri directory inside installDir;
//! 4. in `setup`, before any window exists: empty the web data folder if another runtime wrote
//!    it, then open the vault (which wipes the session folders a crash left behind);
//! 5. create the transparent window and the tray menu window, the tray icon and the global
//!    shortcut, start hot reload, click-through, status sampling and the vault timers, show.
//!
//! The launcher lives in the tray: closing a window hides it, only `exit` quits. On exit the
//! vault is locked. Business logic lives in `genslate-launcher-core` and `genslate-vault`; the
//! modules without Tauri types (`runtime`, `placement`, `vault_service`, ...) are tested on any
//! host, the rest wires them to Tauri.

mod actions;
mod auto_lock;
mod brand;
mod changes;
mod commands;
mod debounce;
mod dialog;
mod elevation;
mod env_bootstrap;
mod error;
mod error_text;
mod events;
mod fatal;
mod files;
mod folders;
mod hotkey;
mod icon_id;
mod icons;
mod identity;
mod monitor;
mod os_open;
mod placement;
mod reload;
mod runtime;
mod settings_write;
mod state;
mod storage_path;
mod tray;
mod tray_menu;
mod vault_commands;
mod vault_dto;
mod vault_service;
mod web_data;
mod webview_host;
mod window;

#[cfg(test)]
mod guards;

use std::sync::Arc;

use genslate_launcher_core::APP_NAME;
use genslate_launcher_core::config::{LogLevel, Settings};
use genslate_paths::AppPaths;
use tauri::utils::config::{AppDirectoriesOverride, AppDirectoryOverrides};
use tauri::{Manager, RunEvent, Runtime, WindowEvent};
use tauri_plugin_log::{Target, TargetKind};

use crate::fatal::Fatal;
use crate::state::{Launcher, VaultSlot};
use crate::vault_dto::VaultErrorDto;
use crate::vault_service::{EXIT_SAVE_BOUND, VaultService};

/// Builds and runs the app. A start-up failure is shown in a message box and ends the process
/// with status 1.
pub fn run() {
    if let Err(fatal) = try_run() {
        dialog::show(&fatal);
        std::process::exit(1);
    }
}

fn try_run() -> Result<(), Fatal> {
    elevation::refuse_if_elevated(elevation::is_elevated())?;
    let paths =
        genslate_paths::resolve(APP_NAME).map_err(|error| Fatal::Paths(error.to_string()))?;
    paths
        .create_dirs()
        .map_err(|error| Fatal::Paths(error.to_string()))?;

    // The runtime and the environment come first: `TEMP` and the `WebView2` variables are read
    // when the first webview is created, and nothing of ours is running yet that could race the
    // change.
    let host = webview_host::installed_version();
    let bundled = runtime::find_bundled(&paths.bundled_webview2_root());
    let chosen = runtime::select(host, bundled, runtime::MIN_HOST_VERSION)?;
    env_bootstrap::apply(&env_bootstrap::plan(&paths, &chosen));
    // The loader is the judge: it honours the variables just set, so this proves the chosen
    // runtime really starts (it fails for a damaged install or an elevated process).
    tauri::webview_version().map_err(|_| Fatal::RuntimeFailed)?;
    let stamp = web_data::stamp_text(&chosen, host);

    let mut context = tauri::generate_context!();
    keep_inside_install_dir(&mut context, &paths);
    let log_level = Settings::load(&paths.config_dir).config.logging.level;

    let app = tauri::Builder::default()
        // Registered first, so a second launch on this drive shows the running launcher and
        // exits before it touches the web data or the vault.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Err(error) = window::show(app) {
                log::warn!("could not show the launcher: {}", error.message());
            }
        }))
        .plugin(log_plugin(&paths, log_level))
        .plugin(hotkey::plugin())
        .register_uri_scheme_protocol(icons::SCHEME, |context, request| {
            icons::handle(context, request)
        })
        .setup(move |app| {
            let handle = app.handle();
            log::info!(
                "{} {} ({:?} mode)",
                app.package_info().name,
                app.package_info().version,
                paths.mode()
            );
            // Before any window exists: a profile another runtime wrote is emptied, and the
            // vault wipes the decrypted files a crash or a pulled drive left behind.
            match web_data::reset_if_runtime_changed(&paths, &stamp) {
                Ok(true) => log::info!("the WebView2 data folder was reset for the new runtime"),
                Ok(false) => {}
                Err(error) => log::warn!("could not reset the WebView2 data folder: {error}"),
            }
            let vault = open_vault(&paths);
            app.manage(Launcher::new(paths.clone(), vault));
            let launcher = handle.state::<Launcher>();
            if let Some(issue) = &launcher.settings().issue {
                log::warn!("settings: {issue}; using the previous values");
            }

            window::create(handle, &launcher)?;
            // A tray menu that fails to build costs the right-click menu, not the launcher.
            if let Err(error) = tray_menu::create(handle, &launcher) {
                log::warn!("the tray menu window could not be created: {}", error.message());
            }
            tray::create(handle)?;
            hotkey::register(handle, &launcher.settings().keybindings.global.toggle);
            reload::start(handle);
            window::spawn_hit_test(handle.clone());
            monitor::spawn(handle.clone());
            vault_commands::spawn_background(handle.clone());
            window::show(handle)?;
            Ok(())
        })
        .on_window_event(on_window_event)
        .invoke_handler(tauri::generate_handler![
            actions::run_action,
            commands::get_context,
            commands::list_apps,
            commands::rescan,
            commands::launch_app,
            commands::open_app_folder,
            commands::set_app_override,
            commands::open_folder,
            commands::open_config_file,
            commands::set_setting,
            commands::get_volume_info,
            commands::set_telemetry_active,
            commands::window_set_pinned,
            commands::window_set_expanded,
            commands::window_set_popup_open,
            commands::window_hide,
            commands::window_show,
            commands::tray_menu_hide,
            commands::quit,
            vault_commands::vault_status,
            vault_commands::vault_create,
            vault_commands::vault_unlock,
            vault_commands::vault_lock,
            vault_commands::vault_change_password,
            vault_commands::vault_list,
            vault_commands::vault_import,
            vault_commands::vault_export,
            vault_commands::vault_open,
            vault_commands::vault_verify,
        ])
        .build(context)
        .map_err(|error| Fatal::Startup(error.to_string()))?;

    app.run(|app, event| match event {
        // The launcher lives in the tray: closing windows never quits, only `exit()` does.
        RunEvent::ExitRequested {
            api, code: None, ..
        } => api.prevent_exit(),
        RunEvent::Exit => lock_vault_for_exit(app),
        _ => {}
    });
    Ok(())
}

/// Points every directory Tauri would otherwise take from the OS into installDir, and gives
/// this drive its own single-instance identity (two launchers on two drives must not share one).
fn keep_inside_install_dir(context: &mut tauri::Context, paths: &AppPaths) {
    let config = context.config_mut();
    config.app.app_directories_override =
        Some(AppDirectoriesOverride::Directories(AppDirectoryOverrides {
            config: Some(paths.config_dir.clone()),
            data: Some(paths.database_dir.clone()),
            local_data: Some(paths.cache_dir.clone()),
            cache: Some(paths.cache_dir.clone()),
            log: Some(paths.log_dir.clone()),
        }));
    config.identifier = identity::install_identifier(&config.identifier, &paths.layout.root);
}

/// Opens the vault; a launcher that cannot (another one holds it, a damaged header) still runs
/// and answers every vault command with the reason.
fn open_vault(paths: &AppPaths) -> VaultSlot {
    match VaultService::open(paths) {
        Ok((service, report)) => {
            if report.stale_files_wiped > 0 {
                log::info!(
                    "wiped {} decrypted files left in the session folder",
                    report.stale_files_wiped
                );
            }
            if !report.stale_files_left.is_empty() {
                log::warn!(
                    "{} session entries could not be wiped",
                    report.stale_files_left.len()
                );
            }
            VaultSlot::Ready(Arc::new(service))
        }
        Err(error) => {
            log::warn!("the vault is unavailable: {error}");
            VaultSlot::Unavailable(VaultErrorDto::from(error))
        }
    }
}

/// Window events: blur hides, anything that moves the launcher window puts it back, and the
/// close button (Alt+F4) hides instead of destroying a window the tray still needs.
fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    let app = window.app_handle();
    match (window.label(), event) {
        (window::MAIN, WindowEvent::Focused(false)) => window::on_blur(app),
        (
            window::MAIN,
            WindowEvent::Moved(_)
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. },
        ) => window::keep_in_place(app),
        (window::MAIN, WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            if let Err(error) = window::hide(app) {
                log::warn!("could not hide the launcher: {}", error.message());
            }
        }
        (tray_menu::TRAY_MENU, WindowEvent::Focused(false)) => tray_menu::on_blur(app),
        (tray_menu::TRAY_MENU, WindowEvent::CloseRequested { api, .. }) => {
            api.prevent_close();
            if let Err(error) = tray_menu::hide(app) {
                log::warn!("tray menu: {}", error.message());
            }
        }
        _ => {}
    }
}

/// Locks the vault when the launcher exits: edits to opened files are saved first, for at most
/// [`EXIT_SAVE_BOUND`], then the decrypted files are wiped whatever happened.
fn lock_vault_for_exit<R: Runtime>(app: &tauri::AppHandle<R>) {
    let Some(launcher) = app.try_state::<Launcher>() else {
        return;
    };
    let Some(service) = launcher.vault_service() else {
        return;
    };
    if let Some(report) = service.lock_for_exit(EXIT_SAVE_BOUND)
        && !report.unsynced.is_empty()
    {
        log::warn!(
            "{} edited vault files could not be saved on exit",
            report.unsynced.len()
        );
    }
}

/// Logs to stdout (dev) and to `<log dir>/launcher.log`, inside installDir.
fn log_plugin<R: Runtime>(paths: &AppPaths, level: LogLevel) -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Folder {
                path: paths.log_dir.clone(),
                file_name: Some(APP_NAME.to_owned()),
            }),
        ])
        .level(level_filter(level))
        .max_file_size(5 * 1024 * 1024)
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(5))
        .build()
}

const fn level_filter(level: LogLevel) -> log::LevelFilter {
    match level {
        LogLevel::Off => log::LevelFilter::Off,
        LogLevel::Error => log::LevelFilter::Error,
        LogLevel::Warn => log::LevelFilter::Warn,
        LogLevel::Info => log::LevelFilter::Info,
        LogLevel::Debug => log::LevelFilter::Debug,
        LogLevel::Trace => log::LevelFilter::Trace,
    }
}

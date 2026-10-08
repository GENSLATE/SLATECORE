//! `run_action`: runs a registry action (`genslate_launcher_core::actions`) by id.
//!
//! This is the single entry point a future AI agent or MCP server will use. The registry is the
//! truth for ids and parameter names; actions whose effect is `ui` (`tab`, `tools`, `settings`,
//! `vault`, `help`) or `ai` (`ask`) change only what the UI shows, so the shell answers
//! [`ActionOutcome::Ui`] and the UI does it. Everything else is done here.

use genslate_launcher_core::LauncherError;
use genslate_launcher_core::actions::{self, Effect};
use genslate_launcher_core::catalog::{AppEntry, AppId, AppStatus};
use genslate_launcher_core::metadata::OverridePatch;
use serde::Serialize;
use serde_json::{Map, Value};
use tauri::{AppHandle, Manager};

use crate::commands;
use crate::error::AppError;
use crate::folders::{ConfigFile, SharedFolder};
use crate::settings_write::{SettingKey, theme_for_mode};
use crate::state::Launcher;
use crate::{events, files, reload, window};

/// What happened.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ActionOutcome {
    Done {
        message: Option<String>,
    },
    /// The caller (the UI) performs this action itself.
    Ui {
        id: &'static str,
    },
}

#[tauri::command]
pub fn run_action(
    app: AppHandle,
    id: String,
    params: Map<String, Value>,
) -> Result<ActionOutcome, AppError> {
    let spec = actions::find(&id)?;
    actions::validate(spec, &params)?;
    if matches!(spec.effect, Effect::Ui | Effect::Ai) {
        return Ok(ActionOutcome::Ui { id: spec.id });
    }
    let text = |name: &str| {
        params
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let launcher = app.state::<Launcher>();
    let message = match spec.id {
        "open" => {
            let entry = resolve_app(&launcher, &text("app"))?;
            commands::launch(&app, &entry.id, None)?;
            Some(format!("Opened {}", entry.name))
        }
        "folder" => {
            let folder = SharedFolder::parse(&text("name"))
                .ok_or_else(|| AppError::InvalidArgument("unknown folder".to_owned()))?;
            files::open_shared_folder(&launcher, folder)?;
            None
        }
        "fav" => {
            let entry = resolve_app(&launcher, &text("app"))?;
            let favorite = !entry.favorite;
            commands::apply_override(
                &app,
                &entry.id,
                &OverridePatch {
                    favorite: Some(favorite),
                    ..OverridePatch::default()
                },
            )?;
            Some(format!(
                "{} {} Favorites",
                entry.name,
                if favorite { "added to" } else { "removed from" }
            ))
        }
        "rescan" => {
            launcher.rescan();
            reload::emit(&app, events::CATALOG, ());
            Some("Apps rescanned".to_owned())
        }
        "theme" => {
            commands::write_setting(&app, SettingKey::Theme, theme_for_mode(&text("mode")))?;
            None
        }
        "size" => {
            commands::write_setting(&app, SettingKey::Size, &text("preset"))?;
            None
        }
        "pin" => {
            let pinned = !launcher.window().pinned;
            window::set_pinned(&app, pinned)?;
            Some(if pinned { "Pinned" } else { "Unpinned" }.to_owned())
        }
        "config" => {
            files::open_config(&launcher, ConfigFile::Settings)?;
            None
        }
        "keys" => {
            files::open_config(&launcher, ConfigFile::Keybindings)?;
            None
        }
        "logs" => {
            files::open_config(&launcher, ConfigFile::Logs)?;
            None
        }
        "hide" => {
            window::hide(&app)?;
            None
        }
        "quit" => {
            app.exit(0);
            None
        }
        other => return Err(LauncherError::UnknownAction(other.to_owned()).into()),
    };
    Ok(ActionOutcome::Done { message })
}

/// An app by id (`genslate/explorer`) or by (case-insensitive) name or key; installed ones first.
fn resolve_app(launcher: &Launcher, query: &str) -> Result<AppEntry, AppError> {
    let catalog = launcher.catalog();
    if let Ok(id) = query.parse::<AppId>()
        && let Some(entry) = catalog.find(&id)
    {
        return Ok(entry.clone());
    }
    let wanted = query.trim().to_lowercase();
    let mut matches: Vec<&AppEntry> = catalog
        .apps()
        .iter()
        .filter(|entry| {
            entry.name.to_lowercase() == wanted || entry.id.key.to_lowercase() == wanted
        })
        .collect();
    matches.sort_by_key(|entry| !matches!(entry.status, AppStatus::Ready | AppStatus::Running));
    matches
        .first()
        .map(|entry| (*entry).clone())
        .ok_or_else(|| LauncherError::UnknownApp(query.to_owned()).into())
}

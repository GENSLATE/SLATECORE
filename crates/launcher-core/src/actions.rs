//! The command registry: every launcher action, described as data.
//!
//! The slash bar and the tray menu list and run these today. The same descriptions (id,
//! summary, typed parameters and an [`Effect`]) are what an AI agent or MCP server will be
//! given later, so it can drive the launcher through exactly the actions a person can (gated
//! by effect).

use serde::Serialize;
use serde_json::{Map, Value};

use crate::LauncherError;

/// What running an action does; used to gate automated callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    /// Changes only what the launcher shows.
    Ui,
    /// Shows, hides or pins the window.
    Window,
    /// Reads state (rescans).
    Read,
    /// Starts a program.
    Launch,
    /// Opens a folder or file in another app.
    Open,
    /// Writes the user's config files.
    WritesConfig,
    /// Talks to an AI model (not available yet).
    Ai,
}

/// Type of a parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ParamKind {
    /// An app, by name or id (the UI resolves names).
    App,
    /// One of a fixed set of values.
    Choice {
        /// The accepted values.
        values: &'static [&'static str],
    },
    /// Free text.
    Text,
}

/// One parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamSpec {
    /// The parameter's name.
    pub name: &'static str,
    /// What it is, as the user reads it.
    pub description: &'static str,
    /// Its type.
    #[serde(flatten)]
    pub kind: ParamKind,
    /// Whether the command refuses to run without it.
    pub required: bool,
}

/// One action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSpec {
    /// Stable id, also the slash command (`/open`).
    pub id: &'static str,
    /// Menu title.
    pub title: &'static str,
    /// One-line summary.
    pub description: &'static str,
    /// Its parameters.
    pub params: &'static [ParamSpec],
    /// What it does to the world.
    pub effect: Effect,
}

const APP: ParamSpec = ParamSpec {
    name: "app",
    description: "App name or id",
    kind: ParamKind::App,
    required: true,
};

/// Folders of the portable storage (`storage/users/<profile>/…`) plus the storage root.
pub const FOLDERS: &[&str] = &[
    "desktop",
    "documents",
    "downloads",
    "music",
    "pictures",
    "videos",
    "storage",
];

/// The tools `/tools <tool>` opens. Settings is the only real one in v1; the others open
/// "Feature Coming Soon" teasers.
pub const TOOLS: &[&str] = &["settings", "manager", "storage", "diagnostics", "ai"];

/// The sections of the Settings tool `/settings <section>` can jump to.
pub const SETTINGS_SECTIONS: &[&str] = &["appearance", "behavior", "keybindings", "vault", "about"];

/// Every action, in the order the slash menu lists them.
pub const REGISTRY: &[ActionSpec] = &[
    ActionSpec {
        id: "open",
        title: "Open app",
        description: "Launch an app",
        params: &[APP],
        effect: Effect::Launch,
    },
    ActionSpec {
        id: "folder",
        title: "Open folder",
        description: "Open one of your portable folders",
        params: &[ParamSpec {
            name: "name",
            description: "Folder",
            kind: ParamKind::Choice { values: FOLDERS },
            required: true,
        }],
        effect: Effect::Open,
    },
    ActionSpec {
        id: "tab",
        title: "Switch tab",
        description: "Show GENSLATE, portapps.io or PortableApps.com apps",
        params: &[ParamSpec {
            name: "source",
            description: "Tab",
            kind: ParamKind::Choice {
                values: &["genslate", "portapps", "portableapps"],
            },
            required: true,
        }],
        effect: Effect::Ui,
    },
    ActionSpec {
        id: "fav",
        title: "Toggle favorite",
        description: "Pin an app to Favorites (or unpin it)",
        params: &[APP],
        effect: Effect::WritesConfig,
    },
    ActionSpec {
        id: "rescan",
        title: "Rescan apps",
        description: "Look for new or removed apps",
        params: &[],
        effect: Effect::Read,
    },
    ActionSpec {
        id: "theme",
        title: "Change theme",
        description: "Polar Night (dark), Snow Storm (light) or match the system",
        params: &[ParamSpec {
            name: "mode",
            description: "Theme",
            kind: ParamKind::Choice {
                values: &["system", "dark", "light"],
            },
            required: true,
        }],
        effect: Effect::WritesConfig,
    },
    ActionSpec {
        id: "size",
        title: "Change size",
        description: "Small, medium or large window",
        params: &[ParamSpec {
            name: "preset",
            description: "Size",
            kind: ParamKind::Choice {
                values: &["s", "m", "l"],
            },
            required: true,
        }],
        effect: Effect::WritesConfig,
    },
    ActionSpec {
        id: "pin",
        title: "Pin / unpin",
        description: "Keep the launcher open when it loses focus",
        params: &[],
        effect: Effect::Window,
    },
    ActionSpec {
        id: "tools",
        title: "Tools",
        description: "Open or close the tools view",
        params: &[ParamSpec {
            name: "tool",
            description: "Tool",
            kind: ParamKind::Choice { values: TOOLS },
            required: false,
        }],
        effect: Effect::Ui,
    },
    ActionSpec {
        id: "settings",
        title: "Settings",
        description: "Open the Settings tool",
        params: &[ParamSpec {
            name: "section",
            description: "Section",
            kind: ParamKind::Choice {
                values: SETTINGS_SECTIONS,
            },
            required: false,
        }],
        effect: Effect::Ui,
    },
    ActionSpec {
        id: "vault",
        title: "Vault",
        description: "Unlock, lock or manage the encrypted vault",
        params: &[],
        effect: Effect::Ui,
    },
    ActionSpec {
        id: "config",
        title: "Edit settings",
        description: "Open settings.toml",
        params: &[],
        effect: Effect::Open,
    },
    ActionSpec {
        id: "keys",
        title: "Edit shortcuts",
        description: "Open keybindings.toml",
        params: &[],
        effect: Effect::Open,
    },
    ActionSpec {
        id: "logs",
        title: "Open logs",
        description: "Open the launcher's log folder",
        params: &[],
        effect: Effect::Open,
    },
    ActionSpec {
        id: "ask",
        title: "Ask AI",
        description: "Ask the SLATECORE assistant (coming soon)",
        params: &[ParamSpec {
            name: "prompt",
            description: "Your question",
            kind: ParamKind::Text,
            required: false,
        }],
        effect: Effect::Ai,
    },
    ActionSpec {
        id: "help",
        title: "Help",
        description: "Shortcuts and commands",
        params: &[],
        effect: Effect::Ui,
    },
    ActionSpec {
        id: "hide",
        title: "Hide",
        description: "Hide the launcher",
        params: &[],
        effect: Effect::Window,
    },
    ActionSpec {
        id: "quit",
        title: "Quit",
        description: "Close the launcher (apps keep running)",
        params: &[],
        effect: Effect::Window,
    },
];

/// Looks an action up by id.
pub fn find(id: &str) -> Result<&'static ActionSpec, LauncherError> {
    REGISTRY
        .iter()
        .find(|spec| spec.id == id)
        .ok_or_else(|| LauncherError::UnknownAction(id.to_owned()))
}

/// Checks `params` against the action's spec: required present, choices valid, strings only,
/// no unknown names.
pub fn validate(spec: &ActionSpec, params: &Map<String, Value>) -> Result<(), LauncherError> {
    let invalid = |message: String| LauncherError::InvalidParams {
        action: spec.id.to_owned(),
        message,
    };
    for name in params.keys() {
        if !spec.params.iter().any(|param| param.name == name) {
            return Err(invalid(format!("unknown parameter {name:?}")));
        }
    }
    for param in spec.params {
        match params.get(param.name) {
            None | Some(Value::Null) if param.required => {
                return Err(invalid(format!(
                    "{} is required",
                    param.description.to_lowercase()
                )));
            }
            None | Some(Value::Null) => {}
            Some(Value::String(text)) => {
                if let ParamKind::Choice { values } = param.kind
                    && !values.contains(&text.as_str())
                {
                    return Err(invalid(format!("use one of: {}", values.join(", "))));
                }
                if param.required && text.trim().is_empty() {
                    return Err(invalid(format!(
                        "{} is required",
                        param.description.to_lowercase()
                    )));
                }
            }
            Some(_) => return Err(invalid(format!("{} must be text", param.name))),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn params(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn ids_are_unique_slash_words() {
        let mut ids: Vec<&str> = REGISTRY.iter().map(|spec| spec.id).collect();
        assert!(
            ids.iter()
                .all(|id| id.bytes().all(|b| b.is_ascii_lowercase()))
        );
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), REGISTRY.len());
    }

    #[test]
    fn validates_required_and_choice_params() -> Result<(), LauncherError> {
        let theme = find("theme")?;
        assert!(validate(theme, &params(json!({ "mode": "dark" }))).is_ok());
        assert!(validate(theme, &params(json!({ "mode": "purple" }))).is_err());
        assert!(validate(theme, &params(json!({}))).is_err());
        assert!(validate(theme, &params(json!({ "mode": "dark", "x": 1 }))).is_err());
        assert!(
            validate(find("ask")?, &params(json!({}))).is_ok(),
            "optional"
        );
        assert!(validate(find("open")?, &params(json!({ "app": 3 }))).is_err());
        assert!(find("nope").is_err());
        Ok(())
    }

    #[test]
    fn serialises_for_the_ui_and_agents() -> Result<(), serde_json::Error> {
        let json = serde_json::to_value(REGISTRY)?;
        assert_eq!(json[0]["id"], "open");
        assert_eq!(json[0]["effect"], "launch");
        assert_eq!(json[0]["params"][0]["type"], "app");
        let folder = &json[1]["params"][0];
        assert_eq!(folder["type"], "choice");
        assert_eq!(folder["values"][0], "desktop");
        Ok(())
    }

    #[test]
    fn registry_contains_settings_and_vault_and_tools_choice_values() -> Result<(), LauncherError> {
        let ids: Vec<&str> = REGISTRY.iter().map(|spec| spec.id).collect();
        for id in [
            "open", "folder", "tab", "fav", "rescan", "theme", "size", "pin", "tools", "config",
            "keys", "logs", "ask", "help", "hide", "quit", "settings", "vault",
        ] {
            assert!(ids.contains(&id), "missing /{id}");
        }
        assert_eq!(ids.len(), 18, "{ids:?}");

        let tools = find("tools")?;
        let [tool] = tools.params else {
            return Err(LauncherError::UnknownAction(
                "tools takes one parameter".to_owned(),
            ));
        };
        assert_eq!(
            tool.kind,
            ParamKind::Choice {
                values: &["settings", "manager", "storage", "diagnostics", "ai"]
            }
        );
        assert!(!tool.required, "/tools alone still toggles the tools view");
        assert!(validate(tools, &params(json!({ "tool": "diagnostics" }))).is_ok());
        assert!(validate(tools, &params(json!({}))).is_ok());
        assert!(validate(tools, &params(json!({ "tool": "nope" }))).is_err());

        assert_eq!(find("settings")?.effect, Effect::Ui);
        assert_eq!(find("vault")?.effect, Effect::Ui);
        assert_eq!(find("vault")?.params, []);
        let settings = find("settings")?;
        assert!(validate(settings, &params(json!({ "section": "vault" }))).is_ok());
        assert!(validate(settings, &params(json!({}))).is_ok());
        assert!(validate(settings, &params(json!({ "section": "nope" }))).is_err());
        Ok(())
    }
}

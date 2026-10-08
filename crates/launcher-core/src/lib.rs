//! SLATECORE LAUNCHER logic: plain Rust, no Tauri, fully unit-tested.
//!
//! - [`config`]: `settings.toml` / `keybindings.toml` (hand-editable, hot-reloaded; an invalid
//!   file keeps the previous settings and reports an issue).
//! - [`catalog`]: GENSLATE apps plus user-installed portapps.io and PortableApps.com apps.
//! - [`metadata`]: per-app metadata and per-tab overrides (comment-preserving writes).
//! - [`launch`]: validated, detached app launching (ids in, never paths).
//! - [`actions`]: the command registry behind the slash bar and the tray menu, and later AI
//!   agents.
//! - [`geometry`]: bottom-right placement on the monitor under the cursor.
//! - [`system`]: drive space, temperatures, usage, running processes.
//! - [`recent`], [`watch`]: launch history and file watching.
//!
//! Everything lives under the install folder resolved by `genslate-paths`; no path is ever
//! persisted, so the folder can move to another drive letter between runs.
#![forbid(unsafe_code)]

pub mod actions;
pub mod catalog;
pub mod config;
mod error;
pub mod geometry;
pub mod launch;
pub mod metadata;
pub mod recent;
pub mod system;
pub mod watch;

pub use error::LauncherError;

/// The launcher's app name in `genslate-paths` (`other/launcher/…`) and its folder key in
/// `programs/genslate/`.
pub const APP_NAME: &str = "launcher";

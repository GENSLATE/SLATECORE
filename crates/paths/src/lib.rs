//! Resolves where SLATECORE LAUNCHER keeps its configs, cache, logs, database and user files.
//!
//! The launcher is portable: everything it controls lives inside its install folder
//! (`installDir`) and nothing is ever written to the PC's user folders. This crate therefore
//! has no operating-system fallback. If the install folder cannot be found, cannot be written
//! to, or is a network (UNC) path, resolving fails with a [`PathsError`] and the caller shows a
//! message and exits.
//!
//! | [`Mode`] | When                                                   | `root`                 |
//! |----------|--------------------------------------------------------|------------------------|
//! | `Suite`  | the exe is inside `<installDir>/programs/…`            | `installDir`           |
//! | `Dev`    | debug build running from a SLATECORE checkout          | the repository         |
//!
//! Both modes have the same shape, so the rest of the code never cares where it runs:
//!
//! ```text
//! <installDir>/
//! ├─ programs/   genslate/<app>/, portableapps.com/, portapps.io/
//! ├─ other/      <app>/{configs,cache,database,logs,documents,licenses,resources}, shared/
//! └─ storage/    users/<profile>/{Desktop,Documents,…}, vault/
//! ```
//!
//! In `Dev` mode `programs/` and `storage/` come from the mock-up install folder
//! `programs/desktop/launcher/installDir` and `other/` from `programs/desktop/launcher/other`.
//!
//! Inside `other/<app>/cache/` live the `WebView2` data folder (`webview2/`), the replacement for
//! `%TEMP%` (`tmp/`) and the vault's session folder (`vault-session/`). The optional bundled
//! `WebView2` runtime is unpacked to `other/shared/webview2/`.
//!
//! No path found here is meant to be saved. Drive letters change between PCs, so persisted
//! state must hold paths relative to `root`.
//!
//! [`resolve`] reads the real environment; [`resolve_with`] and [`detect_layout`] take an
//! explicit [`Environment`] so every mode is unit-testable.
#![forbid(unsafe_code)]

mod environment;
mod error;
mod layout;
mod names;
mod repo;
mod resolve;
mod unc;

pub use environment::{Environment, INSTALL_DIR_ENV, REPO_ROOT_ENV};
pub use error::PathsError;
pub use layout::{Layout, Mode, SHARED_PROFILE, detect_layout, find_install_dir};
pub use repo::find_repo_root;
pub use resolve::{AppPaths, resolve, resolve_with};

//! SLATECORE encrypted vault (`storage/vault`).
//!
//! A random 256-bit vault key is wrapped by an Argon2id key derived from the password
//! (m = 128 MiB, t = 3, p = 1 by default). Each file is a write-once blob `files/<32 hex>.gvf`
//! encrypted with streaming XChaCha20-Poly1305; logical names and folders exist only inside one
//! encrypted index. The header and the index each have two generation-numbered slots, so a
//! crash never leaves the vault unreadable. Opened files are decrypted into a session folder
//! that is synced back and wiped on lock. Design and threat model: `research/vault.md`.
//!
//! Use [`Vault`]; the [`format`], [`crypto`], [`kdf`] and [`throttle`] modules expose the pure
//! building blocks for known-answer tests and tooling.
//!
//! What this does not protect against (state it in the UI): a forgotten password (no recovery),
//! malware on the PC while unlocked, plaintext left on flash media by the session folder or by
//! other programs, an attacker holding an old header copy and the old password after a password
//! change, and rollback of the whole vault folder. The wrong-password throttle only slows
//! guessing through the launcher, never an offline attack.
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub use secrecy::{ExposeSecret, SecretBox, SecretString};

pub mod crypto;
mod error;
pub mod format;
pub mod kdf;
mod path;
mod session;
mod store;
pub mod throttle;
mod vault;

pub use error::VaultError;
pub use kdf::KdfParams;
pub use path::VaultPath;
pub use vault::Vault;

/// Where the vault and its session folder live, and the KDF used for new headers.
#[derive(Clone, Debug)]
pub struct VaultConfig {
    /// `<installDir>/storage/vault`.
    pub root: PathBuf,
    /// `<installDir>/other/launcher/cache/vault-session`. Everything inside is wiped on open
    /// and lock, so it must be a folder the vault owns.
    pub session_root: PathBuf,
    /// Used by `create`, `change_password` and the unlock-time upgrade.
    pub kdf: KdfParams,
    /// `Some(0xFFFF_FFFF)` on FAT32.
    pub max_blob_bytes: Option<u64>,
}

impl VaultConfig {
    /// A configuration with [`KdfParams::STANDARD`] and no file-size limit.
    pub fn new(root: impl Into<PathBuf>, session_root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            session_root: session_root.into(),
            kdf: KdfParams::STANDARD,
            max_blob_bytes: None,
        }
    }
}

/// File or folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
}

/// One vault entry as listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryInfo {
    pub path: VaultPath,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: SystemTime,
}

/// Lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultState {
    Uninitialized,
    Locked,
    Unlocked,
}

/// A cheap snapshot of the vault's state (no I/O, never runs the KDF).
#[derive(Clone, Debug)]
pub struct VaultStatus {
    pub state: VaultState,
    pub kdf: Option<KdfParams>,
    pub kdf_upgrade_pending: bool,
    pub failed_attempts: u32,
    pub retry_after: Option<Duration>,
    pub foreign_items: usize,
    /// Only while unlocked.
    pub entry_count: Option<usize>,
    pub session_files: usize,
}

/// What [`Vault::open`] cleaned up from a previous run.
#[derive(Clone, Debug, Default)]
pub struct StartupReport {
    pub stale_files_wiped: usize,
    pub stale_files_left: Vec<PathBuf>,
}

/// How [`Vault::lock`] treats edits it cannot save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockPolicy {
    /// Refuse to lock (`UnsyncedEdits`) and stay unlocked.
    SyncThenWipe,
    /// Report them in `unsynced` and lock anyway.
    Force,
}

/// The outcome of a lock.
#[derive(Clone, Debug, Default)]
pub struct LockReport {
    pub synced: Vec<VaultPath>,
    pub unsynced: Vec<VaultPath>,
    pub wiped_files: usize,
    pub undeletable: Vec<PathBuf>,
}

/// The outcome of a session sync.
#[derive(Clone, Debug, Default)]
pub struct SyncReport {
    pub updated: Vec<VaultPath>,
    pub imported_new: Vec<VaultPath>,
    pub failed: Vec<(VaultPath, String)>,
}

/// What an import does when the name is taken (compared case-insensitively).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conflict {
    Fail,
    Replace,
    KeepBoth,
}

/// The outcome of [`Vault::verify`].
#[derive(Clone, Debug, Default)]
pub struct VerifyReport {
    pub files_checked: usize,
    pub problems: Vec<(VaultPath, Problem)>,
    pub orphan_blobs: usize,
}

/// A problem `verify` found with one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    BlobMissing,
    Tampered,
    Truncated,
    SizeMismatch,
}

/// (`bytes_done`, `bytes_total`); must be cheap.
pub type ProgressFn<'a> = &'a (dyn Fn(u64, u64) + Send + Sync);

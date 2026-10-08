//! The vault's single error type.
//!
//! No variant ever carries a password, a key or a file name taken from the encrypted index.
//! `NotFound` and `Exists` carry the path the caller supplied, nothing more.

use std::fmt;
use std::time::Duration;

use crate::path::VaultPath;

/// Everything a vault operation can fail with.
///
/// `Debug` is written by hand so that [`VaultError::UnsyncedEdits`] prints a count instead of
/// the names it carries for the UI.
#[derive(thiserror::Error)]
#[non_exhaustive]
pub enum VaultError {
    #[error("the vault has not been created yet")]
    Uninitialized,
    #[error("a vault already exists here")]
    AlreadyExists,
    #[error("the vault is locked")]
    Locked,
    #[error("the vault is already unlocked")]
    AlreadyUnlocked,
    #[error("wrong password")]
    WrongPassword,
    #[error("too many wrong passwords, try again in {retry_after:?}")]
    Throttled { retry_after: Duration },
    #[error("password rejected: {0}")]
    PasswordRejected(&'static str),
    #[error("the vault is in use by another launcher instance")]
    Busy,
    #[error("the vault header is damaged")]
    HeaderDamaged,
    #[error("unsupported vault format version {0}")]
    UnsupportedVersion(u16),
    #[error("unsupported key-derivation parameters")]
    UnsupportedKdf,
    #[error("not enough memory to unlock (needs about {needed_mib} MiB)")]
    OutOfMemory { needed_mib: u32 },
    #[error("vault data failed authentication ({0})")]
    Tampered(&'static str),
    #[error("encrypted file is missing from the vault folder")]
    BlobMissing,
    #[error("no such entry: {0}")]
    NotFound(String),
    #[error("entry already exists: {0}")]
    Exists(String),
    #[error("invalid vault path: {0}")]
    InvalidPath(&'static str),
    #[error("vault limit reached: {0}")]
    LimitExceeded(&'static str),
    #[error("file is too large for this drive's file system")]
    FileTooLarge,
    #[error("operation cancelled because the vault was locked")]
    Cancelled,
    #[error("{} edited file(s) could not be saved", .0.len())]
    UnsyncedEdits(Vec<VaultPath>),
    #[error("I/O error while {context}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("internal error: {0}")]
    Internal(&'static str),
}

impl fmt::Debug for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uninitialized => f.write_str("Uninitialized"),
            Self::AlreadyExists => f.write_str("AlreadyExists"),
            Self::Locked => f.write_str("Locked"),
            Self::AlreadyUnlocked => f.write_str("AlreadyUnlocked"),
            Self::WrongPassword => f.write_str("WrongPassword"),
            Self::Throttled { retry_after } => f
                .debug_struct("Throttled")
                .field("retry_after", retry_after)
                .finish(),
            Self::PasswordRejected(why) => f.debug_tuple("PasswordRejected").field(why).finish(),
            Self::Busy => f.write_str("Busy"),
            Self::HeaderDamaged => f.write_str("HeaderDamaged"),
            Self::UnsupportedVersion(v) => f.debug_tuple("UnsupportedVersion").field(v).finish(),
            Self::UnsupportedKdf => f.write_str("UnsupportedKdf"),
            Self::OutOfMemory { needed_mib } => f
                .debug_struct("OutOfMemory")
                .field("needed_mib", needed_mib)
                .finish(),
            Self::Tampered(what) => f.debug_tuple("Tampered").field(what).finish(),
            Self::BlobMissing => f.write_str("BlobMissing"),
            Self::NotFound(path) => f.debug_tuple("NotFound").field(path).finish(),
            Self::Exists(path) => f.debug_tuple("Exists").field(path).finish(),
            Self::InvalidPath(why) => f.debug_tuple("InvalidPath").field(why).finish(),
            Self::LimitExceeded(what) => f.debug_tuple("LimitExceeded").field(what).finish(),
            Self::FileTooLarge => f.write_str("FileTooLarge"),
            Self::Cancelled => f.write_str("Cancelled"),
            Self::UnsyncedEdits(paths) => f
                .debug_tuple("UnsyncedEdits")
                .field(&format_args!("{} path(s)", paths.len()))
                .finish(),
            Self::Io { context, source } => f
                .debug_struct("Io")
                .field("context", context)
                .field("source", source)
                .finish(),
            Self::Internal(what) => f.debug_tuple("Internal").field(what).finish(),
        }
    }
}

impl VaultError {
    /// Wraps an I/O error with a short, static description of what was being done.
    pub(crate) fn io(context: &'static str) -> impl FnOnce(std::io::Error) -> Self {
        move |source| Self::Io { context, source }
    }
}

/// Reasons attached to [`VaultError::Tampered`]. Kept in one place so [`crate::Problem`]
/// classification in `verify` matches what the readers report.
pub(crate) mod tamper {
    pub(crate) const BLOB: &str = "encrypted file";
    pub(crate) const BLOB_HEADER: &str = "encrypted file header";
    pub(crate) const BLOB_ID: &str = "encrypted file id";
    pub(crate) const TRUNCATED: &str = "truncated encrypted file";
    pub(crate) const SIZE: &str = "encrypted file size";
    pub(crate) const INDEX: &str = "index";
}

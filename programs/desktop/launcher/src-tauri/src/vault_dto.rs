//! What the vault commands send to the webview and take from it, as the frontend's
//! `launcher.types.ts` spells it (camelCase fields, upper-case error codes).
//!
//! Nothing here carries a password or a key, and no error carries an OS path: the vault's own
//! errors are paths-free, and file lists report session files relative to the session folder.

use std::path::Path;
use std::time::UNIX_EPOCH;

use genslate_vault::{
    Conflict, EntryInfo, EntryKind, KdfParams, LockReport, Problem, VaultError, VaultState,
    VaultStatus, VerifyReport,
};
use serde::{Deserialize, Serialize};

/// `uninitialized`, `locked` or `unlocked`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StateDto {
    Uninitialized,
    Locked,
    Unlocked,
}

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfParamsDto {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl From<KdfParams> for KdfParamsDto {
    fn from(kdf: KdfParams) -> Self {
        Self {
            m_cost_kib: kdf.m_cost_kib,
            t_cost: kdf.t_cost,
            p_cost: kdf.p_cost,
        }
    }
}

/// The vault's state (`vault_status` and every `vault://status` event).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatusDto {
    pub state: StateDto,
    pub kdf: Option<KdfParamsDto>,
    pub kdf_upgrade_pending: bool,
    pub failed_attempts: u32,
    pub retry_after_ms: Option<u64>,
    pub foreign_items: usize,
    pub entry_count: Option<usize>,
    pub session_files: usize,
}

fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

impl From<&VaultStatus> for VaultStatusDto {
    fn from(status: &VaultStatus) -> Self {
        Self {
            state: match status.state {
                VaultState::Uninitialized => StateDto::Uninitialized,
                VaultState::Locked => StateDto::Locked,
                VaultState::Unlocked => StateDto::Unlocked,
            },
            kdf: status.kdf.map(Into::into),
            kdf_upgrade_pending: status.kdf_upgrade_pending,
            failed_attempts: status.failed_attempts,
            retry_after_ms: status.retry_after.map(millis),
            foreign_items: status.foreign_items,
            entry_count: status.entry_count,
            session_files: status.session_files,
        }
    }
}

impl VaultStatusDto {
    /// What the UI sees when the vault could not be opened at start-up: locked, nothing in it.
    /// Every command that needs the vault then reports why.
    pub const fn unavailable() -> Self {
        Self {
            state: StateDto::Locked,
            kdf: None,
            kdf_upgrade_pending: false,
            failed_attempts: 0,
            retry_after_ms: None,
            foreign_items: 0,
            entry_count: None,
            session_files: 0,
        }
    }
}

/// `file` or `dir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KindDto {
    File,
    Dir,
}

/// One listed entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDto {
    /// Vault path with `/` separators.
    pub path: String,
    pub kind: KindDto,
    pub size: u64,
    pub modified_ms: u64,
}

impl From<&EntryInfo> for EntryDto {
    fn from(entry: &EntryInfo) -> Self {
        Self {
            path: entry.path.as_str().to_owned(),
            kind: match entry.kind {
                EntryKind::File => KindDto::File,
                EntryKind::Dir => KindDto::Dir,
            },
            size: entry.size,
            modified_ms: entry.modified.duration_since(UNIX_EPOCH).map_or(0, millis),
        }
    }
}

/// The outcome of a lock. `undeletable` holds session files relative to the session folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockReportDto {
    pub synced: Vec<String>,
    pub unsynced: Vec<String>,
    pub wiped_files: usize,
    pub undeletable: Vec<String>,
}

impl LockReportDto {
    /// Converts a vault report; `session_root` is stripped from the paths it could not wipe.
    pub fn new(report: &LockReport, session_root: &Path) -> Self {
        Self {
            synced: report
                .synced
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            unsynced: report
                .unsynced
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            wiped_files: report.wiped_files,
            undeletable: report
                .undeletable
                .iter()
                .map(|path| relative_to(path, session_root))
                .collect(),
        }
    }
}

/// `path` below `root` with `/` separators, or just its file name when it is not below `root`.
pub fn relative_to(path: &Path, root: &Path) -> String {
    let relative = path
        .strip_prefix(root)
        .unwrap_or_else(|_| path.file_name().map_or(path, Path::new));
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// A file `verify` found damaged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProblemDto {
    pub path: String,
    pub problem: &'static str,
}

/// The outcome of `verify`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReportDto {
    pub files_checked: usize,
    pub orphan_blobs: usize,
    pub problems: Vec<ProblemDto>,
}

impl From<&VerifyReport> for VerifyReportDto {
    fn from(report: &VerifyReport) -> Self {
        Self {
            files_checked: report.files_checked,
            orphan_blobs: report.orphan_blobs,
            problems: report
                .problems
                .iter()
                .map(|(path, problem)| ProblemDto {
                    path: path.as_str().to_owned(),
                    problem: match problem {
                        Problem::BlobMissing => "blobMissing",
                        Problem::Tampered => "tampered",
                        Problem::Truncated => "truncated",
                        Problem::SizeMismatch => "sizeMismatch",
                    },
                })
                .collect(),
        }
    }
}

/// What `vault_open` returns: where the decrypted copy is, relative to the session folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedDto {
    pub session_path: String,
}

/// What an import does when a name is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictDto {
    Fail,
    Replace,
    KeepBoth,
}

impl From<ConflictDto> for Conflict {
    fn from(conflict: ConflictDto) -> Self {
        match conflict {
            ConflictDto::Fail => Self::Fail,
            ConflictDto::Replace => Self::Replace,
            ConflictDto::KeepBoth => Self::KeepBoth,
        }
    }
}

/// A rejected vault command. `message` never contains a password, a key or an OS path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultErrorDto {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
}

impl VaultErrorDto {
    fn plain(code: &'static str, message: String) -> Self {
        Self {
            code,
            message,
            retry_after_ms: None,
            paths: None,
        }
    }

    /// The file was decrypted into the session folder but the system could not open it.
    pub fn open_failed() -> Self {
        Self::plain(
            "IO",
            "the file was decrypted but could not be opened with its program".to_owned(),
        )
    }
}

impl From<&VaultError> for VaultErrorDto {
    fn from(error: &VaultError) -> Self {
        let message = error.to_string();
        let code = match error {
            VaultError::Uninitialized => "UNINITIALIZED",
            VaultError::AlreadyExists => "ALREADY_EXISTS",
            VaultError::Locked => "LOCKED",
            VaultError::AlreadyUnlocked => "ALREADY_UNLOCKED",
            VaultError::WrongPassword => "WRONG_PASSWORD",
            VaultError::Throttled { retry_after } => {
                return Self {
                    retry_after_ms: Some(millis(*retry_after)),
                    ..Self::plain("THROTTLED", message)
                };
            }
            VaultError::PasswordRejected(_) => "PASSWORD_REJECTED",
            VaultError::Busy => "BUSY",
            VaultError::HeaderDamaged => "HEADER_DAMAGED",
            VaultError::UnsupportedVersion(_) => "UNSUPPORTED_VERSION",
            VaultError::UnsupportedKdf => "UNSUPPORTED_KDF",
            VaultError::OutOfMemory { .. } => "OUT_OF_MEMORY",
            VaultError::Tampered(_) => "TAMPERED",
            VaultError::BlobMissing => "BLOB_MISSING",
            VaultError::NotFound(_) => "NOT_FOUND",
            VaultError::Exists(_) => "EXISTS",
            VaultError::InvalidPath(_) => "INVALID_PATH",
            VaultError::LimitExceeded(_) => "LIMIT_EXCEEDED",
            VaultError::FileTooLarge => "FILE_TOO_LARGE",
            VaultError::Cancelled => "CANCELLED",
            VaultError::UnsyncedEdits(paths) => {
                return Self {
                    paths: Some(paths.iter().map(|p| p.as_str().to_owned()).collect()),
                    ..Self::plain("UNSYNCED_EDITS", message)
                };
            }
            VaultError::Io { .. } => "IO",
            _ => "INTERNAL",
        };
        Self::plain(code, message)
    }
}

impl From<VaultError> for VaultErrorDto {
    fn from(error: VaultError) -> Self {
        Self::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_vault::VaultPath;
    use std::error::Error;
    use std::path::PathBuf;
    use std::time::Duration;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn status_serialises_as_the_frontend_expects() -> TestResult {
        let status = VaultStatus {
            state: VaultState::Locked,
            kdf: Some(KdfParams::STANDARD),
            kdf_upgrade_pending: false,
            failed_attempts: 2,
            retry_after: Some(Duration::from_millis(1500)),
            foreign_items: 3,
            entry_count: None,
            session_files: 0,
        };
        let json = serde_json::to_value(VaultStatusDto::from(&status))?;
        assert_eq!(json["state"], "locked");
        assert_eq!(json["kdf"]["mCostKib"], 131_072);
        assert_eq!(json["kdf"]["tCost"], 3);
        assert_eq!(json["retryAfterMs"], 1500);
        assert_eq!(json["foreignItems"], 3);
        assert!(json["entryCount"].is_null());
        assert_eq!(json["sessionFiles"], 0);
        assert_eq!(json["failedAttempts"], 2);
        assert_eq!(json["kdfUpgradePending"], false);
        Ok(())
    }

    #[test]
    fn entries_use_millis_and_lowercase_kinds() -> TestResult {
        let entry = EntryInfo {
            path: VaultPath::parse("Notes/ideas.md")?,
            kind: EntryKind::File,
            size: 42,
            modified: UNIX_EPOCH + Duration::from_millis(1234),
        };
        let json = serde_json::to_value(EntryDto::from(&entry))?;
        assert_eq!(json["path"], "Notes/ideas.md");
        assert_eq!(json["kind"], "file");
        assert_eq!(json["modifiedMs"], 1234);
        Ok(())
    }

    #[test]
    fn errors_carry_a_code_and_never_a_path() -> TestResult {
        let wrong = serde_json::to_value(VaultErrorDto::from(VaultError::WrongPassword))?;
        assert_eq!(wrong["code"], "WRONG_PASSWORD");
        assert!(wrong.get("retryAfterMs").is_none());

        let throttled = VaultError::Throttled {
            retry_after: Duration::from_secs(30),
        };
        let json = serde_json::to_value(VaultErrorDto::from(&throttled))?;
        assert_eq!(json["code"], "THROTTLED");
        assert_eq!(json["retryAfterMs"], 30_000);

        let unsynced = VaultError::UnsyncedEdits(vec![VaultPath::parse("a/b.txt")?]);
        let json = serde_json::to_value(VaultErrorDto::from(&unsynced))?;
        assert_eq!(json["code"], "UNSYNCED_EDITS");
        assert_eq!(json["paths"][0], "a/b.txt");

        let io = VaultError::Io {
            context: "writing the exported file",
            source: std::io::Error::other("E:\\secret\\path denied"),
        };
        let dto = VaultErrorDto::from(&io);
        assert_eq!(dto.code, "IO");
        assert!(!dto.message.contains("secret"), "{}", dto.message);
        Ok(())
    }

    #[test]
    fn every_code_is_one_the_frontend_knows() {
        const KNOWN: [&str; 23] = [
            "UNINITIALIZED",
            "ALREADY_EXISTS",
            "LOCKED",
            "ALREADY_UNLOCKED",
            "WRONG_PASSWORD",
            "THROTTLED",
            "PASSWORD_REJECTED",
            "BUSY",
            "HEADER_DAMAGED",
            "UNSUPPORTED_VERSION",
            "UNSUPPORTED_KDF",
            "OUT_OF_MEMORY",
            "TAMPERED",
            "BLOB_MISSING",
            "NOT_FOUND",
            "EXISTS",
            "INVALID_PATH",
            "LIMIT_EXCEEDED",
            "FILE_TOO_LARGE",
            "CANCELLED",
            "UNSYNCED_EDITS",
            "IO",
            "INTERNAL",
        ];
        let samples = [
            VaultError::Uninitialized,
            VaultError::AlreadyExists,
            VaultError::Locked,
            VaultError::AlreadyUnlocked,
            VaultError::WrongPassword,
            VaultError::PasswordRejected("x"),
            VaultError::Busy,
            VaultError::HeaderDamaged,
            VaultError::UnsupportedVersion(9),
            VaultError::UnsupportedKdf,
            VaultError::OutOfMemory { needed_mib: 1 },
            VaultError::Tampered("x"),
            VaultError::BlobMissing,
            VaultError::NotFound("x".into()),
            VaultError::Exists("x".into()),
            VaultError::InvalidPath("x"),
            VaultError::LimitExceeded("x"),
            VaultError::FileTooLarge,
            VaultError::Cancelled,
            VaultError::Internal("x"),
            VaultError::InvalidConfig("x"),
        ];
        for error in &samples {
            let code = VaultErrorDto::from(error).code;
            assert!(KNOWN.contains(&code), "{code}");
        }
    }

    #[test]
    fn session_files_are_reported_relative_to_the_session_folder() {
        let root = PathBuf::from("/x/vault-session");
        assert_eq!(
            relative_to(&root.join("1a2b3c4d").join("report.docx"), &root),
            "1a2b3c4d/report.docx"
        );
        assert_eq!(
            relative_to(Path::new("/elsewhere/leak.txt"), &root),
            "leak.txt"
        );
    }

    #[test]
    fn the_unavailable_status_is_a_locked_empty_vault() -> TestResult {
        let json = serde_json::to_value(VaultStatusDto::unavailable())?;
        assert_eq!(json["state"], "locked");
        assert_eq!(json["sessionFiles"], 0);
        Ok(())
    }

    #[test]
    fn conflicts_parse_from_the_frontend_spelling() -> TestResult {
        let conflict: ConflictDto = serde_json::from_str("\"keepBoth\"")?;
        assert_eq!(Conflict::from(conflict), Conflict::KeepBoth);
        assert!(serde_json::from_str::<ConflictDto>("\"overwrite\"").is_err());
        Ok(())
    }
}

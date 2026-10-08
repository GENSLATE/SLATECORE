//! The session folder: decrypted working copies, their registry, and wiping
//! (research/vault.md 6).
//!
//! Honest limit: wiping overwrites, truncates, renames and deletes, which is the best a
//! user-mode program can do. Flash wear levelling, file-system journals and the opening
//! program's own copies can still hold plaintext.

// cspell:ignore crdownload rfind

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::crypto;
use crate::error::VaultError;
use crate::path::VaultPath;

/// Longest session path third-party Windows programs reliably open (`MAX_PATH` minus the NUL).
const MAX_SESSION_PATH_UNITS: usize = 259;
/// Longest extension kept when a name has to be shortened.
const MAX_KEPT_EXTENSION_UNITS: usize = 16;

/// One decrypted working copy.
#[derive(Clone)]
pub(crate) struct SessionFile {
    /// Stable id: the vault entry can be renamed while the copy is open.
    pub(crate) id: u64,
    pub(crate) entry: VaultPath,
    pub(crate) file: PathBuf,
    /// `BLAKE2b-256` of the plaintext last known to be in the vault.
    pub(crate) hash: [u8; 32],
    /// `(length, mtime)` when the copy was last known to match `hash`.
    pub(crate) baseline: Baseline,
}

impl SessionFile {
    pub(crate) fn folder(&self) -> Option<&Path> {
        self.file.parent()
    }
}

/// Session state for one unlock: a random id and the registry (memory only).
pub(crate) struct Session {
    pub(crate) sid: String,
    pub(crate) next_slot: u32,
    pub(crate) next_id: u64,
    pub(crate) files: Vec<SessionFile>,
}

impl Session {
    pub(crate) fn new() -> Result<Self, VaultError> {
        let sid: [u8; 4] = crypto::random()?;
        Ok(Self {
            sid: hex(&sid),
            next_slot: 1,
            next_id: 1,
            files: Vec::new(),
        })
    }

    /// A fresh `<session_root>/<sid>/<nnnn>` folder path (not created yet).
    pub(crate) fn allocate_folder(&mut self, session_root: &Path) -> PathBuf {
        let slot = self.next_slot;
        self.next_slot += 1;
        session_root.join(&self.sid).join(format!("{slot:04}"))
    }

    pub(crate) fn find_entry(&self, entry: &VaultPath) -> Option<&SessionFile> {
        self.files.iter().find(|f| f.entry == *entry)
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Editor lock and temp files that are never imported as new siblings.
pub(crate) fn is_ignored_sibling(name: &str) -> bool {
    let extension = Path::new(name).extension();
    name.starts_with("~$")
        || name.starts_with(".~lock.")
        || extension
            .is_some_and(|e| e.eq_ignore_ascii_case("tmp") || e.eq_ignore_ascii_case("crdownload"))
}

/// `(length, mtime)` of a file: the cheap "did it change?" check.
pub(crate) type Baseline = (u64, Option<SystemTime>);

/// The [`Baseline`] of a file.
pub(crate) fn stat(meta: &fs::Metadata) -> Baseline {
    (meta.len(), meta.modified().ok())
}

/// The on-disk name for a working copy of `name` inside `folder`, shortened when needed so the
/// whole path stays within 259 UTF-16 units (stem cut, extension kept, `~` + 4 hex added).
/// Outside Windows the name is also kept within 255 UTF-8 bytes (the usual `NAME_MAX`).
pub(crate) fn session_file_name(folder: &Path, name: &str) -> Result<String, VaultError> {
    let folder_units = folder.to_string_lossy().encode_utf16().count() + 1;
    let units_budget = MAX_SESSION_PATH_UNITS.saturating_sub(folder_units);
    let bytes_budget = if cfg!(windows) { usize::MAX } else { 255 };
    let fits = |s: &str| s.encode_utf16().count() <= units_budget && s.len() <= bytes_budget;
    if fits(name) {
        return Ok(name.to_owned());
    }
    let suffix: [u8; 2] = crypto::random()?;
    let suffix = format!("~{}", hex(&suffix));
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 && name[i..].encode_utf16().count() <= MAX_KEPT_EXTENSION_UNITS => {
            name.split_at(i)
        }
        _ => (name, ""),
    };
    let mut short = String::new();
    for c in stem.chars() {
        let candidate_len = short.len() + c.len_utf8();
        let mut candidate = String::with_capacity(candidate_len + suffix.len() + ext.len());
        candidate.push_str(&short);
        candidate.push(c);
        candidate.push_str(&suffix);
        candidate.push_str(ext);
        if !fits(&candidate) {
            break;
        }
        short.push(c);
    }
    let trimmed = short.trim_end_matches([' ', '.']);
    let result = format!("{trimmed}{suffix}{ext}");
    if fits(&result) {
        Ok(result)
    } else {
        Err(VaultError::LimitExceeded(
            "the session folder path is too long",
        ))
    }
}

/// What a wipe achieved.
#[derive(Debug, Default)]
pub(crate) struct WipeReport {
    pub(crate) wiped: usize,
    /// Session files and folders that could not be removed.
    pub(crate) left: Vec<PathBuf>,
    /// Entries in the session root that are not session folders: never touched.
    pub(crate) foreign: Vec<PathBuf>,
}

/// `true` for the name of a session folder (`Session::sid`: 8 lowercase hex digits).
pub(crate) fn is_session_id(name: &str) -> bool {
    name.len() == 8 && name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Wipes every session folder in `session_root`, keeping the root itself. Only entries named
/// like a session id are touched (the root is configurable, so it may hold other things);
/// everything else is listed in [`WipeReport::foreign`]. A missing root is not an error.
pub(crate) fn wipe_sessions(session_root: &Path) -> WipeReport {
    let mut report = WipeReport::default();
    let Ok(entries) = fs::read_dir(session_root) else {
        return report;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let ours = entry.file_name().to_str().is_some_and(is_session_id)
            && entry
                .file_type()
                .is_ok_and(|t| t.is_dir() || t.is_symlink());
        if ours {
            wipe_path(&path, &mut report);
        } else {
            report.foreign.push(path);
        }
    }
    report
}

/// Wipes a file, or a folder and everything below it. Symbolic links and junctions are removed
/// without touching their targets.
pub(crate) fn wipe_path(path: &Path, report: &mut WipeReport) {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(meta) = fs::symlink_metadata(&current) else {
            continue;
        };
        if meta.is_dir() {
            if let Ok(entries) = fs::read_dir(&current) {
                stack.extend(entries.flatten().map(|e| e.path()));
            }
            dirs.push(current);
        } else {
            files.push((current, meta));
        }
    }
    for (file, meta) in files {
        match wipe_file(&file, &meta) {
            Ok(()) => report.wiped += 1,
            Err(_) => report.left.push(file),
        }
    }
    // Deepest folders first.
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for dir in dirs {
        if fs::remove_dir(&dir).is_err() && !report.left.iter().any(|p| p.starts_with(&dir)) {
            report.left.push(dir);
        }
    }
}

/// Removes a directory entry without opening it: a link (never its target) or a folder link.
fn unlink(path: &Path) -> io::Result<()> {
    fs::remove_file(path).or_else(|_| fs::remove_dir(path))
}

/// Overwrite with zeros, sync, truncate, rename to a random name, delete (research/vault.md 6.6).
///
/// Only a plain file with no other name is overwritten: a link, a hard-linked file (Unix; std
/// reports no link count on Windows) or a file swapped for something else after it was listed
/// is only unlinked, so nothing outside the session folder is ever written.
pub(crate) fn wipe_file(path: &Path, meta: &fs::Metadata) -> io::Result<()> {
    if !meta.is_file() || has_other_names(meta) {
        return unlink(path);
    }
    let Some(mut file) = open_for_wipe(path, meta)? else {
        return unlink(path);
    };
    let zeros = vec![0u8; 64 * 1024];
    let mut left = meta.len();
    while left > 0 {
        let n = usize::try_from(left.min(zeros.len() as u64)).unwrap_or(zeros.len());
        file.write_all(&zeros[..n])?;
        left -= n as u64;
    }
    file.sync_all()?;
    file.set_len(0)?;
    drop(file);
    let mut target = path.to_path_buf();
    if let (Some(parent), Ok(random)) = (path.parent(), crypto::random::<8>()) {
        let renamed = parent.join(hex(&random));
        if fs::rename(path, &renamed).is_ok() {
            target = renamed;
        }
    }
    fs::remove_file(&target)
}

/// `true` when the file has another hard link (another name may be outside the session folder).
fn has_other_names(meta: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        meta.nlink() > 1
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

/// Opens `path` for overwriting without following a link, clearing the read-only attribute if
/// needed. `None` when what is now at `path` is not the plain file `listed` described.
fn open_for_wipe(path: &Path, listed: &fs::Metadata) -> io::Result<Option<File>> {
    match open_no_follow(path, true) {
        Ok(file) => return still_the_same(file, listed),
        Err(e)
            if e.kind() != io::ErrorKind::PermissionDenied || !listed.permissions().readonly() =>
        {
            return Err(e);
        }
        Err(_) => {}
    }
    // Read-only: clear the attribute through a handle, so a swapped-in link is not followed.
    let Some(handle) = still_the_same(open_no_follow(path, false)?, listed)? else {
        return Ok(None);
    };
    handle.set_permissions(writable(listed.permissions()))?;
    drop(handle);
    still_the_same(open_no_follow(path, true)?, listed)
}

#[cfg(unix)]
fn open_no_follow(path: &Path, write: bool) -> io::Result<File> {
    // No `O_NOFOLLOW` without libc: a followed link is caught by `still_the_same`, which
    // compares device and inode with what was listed before anything is written.
    OpenOptions::new().read(!write).write(write).open(path)
}

#[cfg(windows)]
fn open_no_follow(path: &Path, write: bool) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
    let mut options = OpenOptions::new();
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    if write {
        options.write(true);
    } else {
        options.access_mode(FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES);
    }
    options.open(path)
}

/// `Some(file)` when the opened handle is the plain, singly linked file that was listed.
fn still_the_same(file: File, listed: &fs::Metadata) -> io::Result<Option<File>> {
    let opened = file.metadata()?;
    if !opened.is_file() || has_other_names(&opened) {
        return Ok(None);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if (opened.dev(), opened.ino()) != (listed.dev(), listed.ino()) {
            return Ok(None);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        let _ = listed;
        if opened.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Ok(None);
        }
    }
    Ok(Some(file))
}

/// `permissions` with writing allowed for the owner.
fn writable(mut permissions: fs::Permissions) -> fs::Permissions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(not(unix))]
    {
        #[allow(clippy::permissions_set_readonly_false)] // Windows: clears FILE_ATTRIBUTE_READONLY
        permissions.set_readonly(false);
    }
    permissions
}

/// Removes a working copy and, when it is then empty, its `<nnnn>` folder.
pub(crate) fn wipe_session_file(file: &SessionFile) {
    let mut report = WipeReport::default();
    wipe_path(&file.file, &mut report);
    if let Some(folder) = file.folder() {
        let _ = fs::remove_dir(folder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_are_kept() {
        let folder = Path::new("/s/abcd1234/0001");
        assert_eq!(
            session_file_name(folder, "report.docx").ok().as_deref(),
            Some("report.docx")
        );
    }

    #[test]
    fn long_names_are_shortened_with_extension_kept() -> Result<(), VaultError> {
        let folder = PathBuf::from(format!("/{}/abcd1234/0001", "x".repeat(120)));
        let name = format!("{}.txt", "L".repeat(251));
        let short = session_file_name(&folder, &name)?;
        let full = folder.join(&short);
        assert!(full.to_string_lossy().encode_utf16().count() <= MAX_SESSION_PATH_UNITS);
        assert!(Path::new(&short).extension().is_some_and(|e| e == "txt"));
        assert!(short.contains('~'));
        let emoji = "\u{1F600}".repeat(127);
        let short = session_file_name(Path::new("/s/abcd1234/0001"), &emoji)?;
        if !cfg!(windows) {
            assert!(short.len() <= 255);
        }
        Ok(())
    }

    #[test]
    fn ignored_siblings() {
        for name in ["~$doc.docx", ".~lock.doc.odt#", "x.TMP", "file.crdownload"] {
            assert!(is_ignored_sibling(name), "{name}");
        }
        assert!(!is_ignored_sibling("copy.txt"));
    }
}

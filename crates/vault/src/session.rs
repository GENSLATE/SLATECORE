//! The session folder: decrypted working copies, their registry, and wiping
//! (research/vault.md 6).
//!
//! Honest limit: wiping overwrites, truncates, renames and deletes, which is the best a
//! user-mode program can do. Flash wear levelling, file-system journals and the opening
//! program's own copies can still hold plaintext.

// cspell:ignore crdownload rfind

use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
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
    write_zeros(&mut file, meta.len())?;
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

/// Overwrites the first `len` bytes of `file` with zeros and syncs.
fn write_zeros(file: &mut File, len: u64) -> io::Result<()> {
    let zeros = vec![0u8; 64 * 1024];
    let mut left = len;
    while left > 0 {
        let n = usize::try_from(left.min(zeros.len() as u64)).unwrap_or(zeros.len());
        file.write_all(&zeros[..n])?;
        left -= n as u64;
    }
    file.sync_all()
}

/// What became of a plain original after its import (see [`wipe_original`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OriginalWipe {
    /// The original was overwritten and is gone.
    Wiped,
    /// Left in place: it is not provably what was encrypted (untouched), or the overwrite
    /// itself failed partway. It stays a foreign item.
    Kept,
}

/// Wipes a file dropped into the vault folder once it is in the vault (research/vault.md 6.6),
/// only while the file at `path` is still exactly what was encrypted: a plain, singly linked file
/// with the `listed` length and time, `len` bytes and content `hash`.
///
/// Unlike [`wipe_file`] (which owns the session folder), nothing at `path` is unlinked unless it
/// is proven to be the emptied original. The content is hashed and overwritten through one
/// handle, on Windows opened without sharing and without following a reparse point, so nobody
/// can write, rename or delete the file between the hash and the overwrite. (Unix has no such
/// lock: a file renamed over the original is not touched, and a change of length or time after
/// the hash is refused before anything is written.) The emptied file is
/// then renamed to a random name next to it and deleted only if what was moved is still the
/// emptied file; a plain file that was moved instead is put back under its name by hard link,
/// anything else is set aside under `<name>.restored-<hex>`.
pub(crate) fn wipe_original(
    path: &Path,
    listed: Baseline,
    len: u64,
    hash: &[u8; 32],
) -> OriginalWipe {
    wipe_original_with(path, listed, len, hash, &mut || {})
}

/// [`wipe_original`], running `between` after the hash matched and before the overwrite.
fn wipe_original_with(
    path: &Path,
    listed: Baseline,
    len: u64,
    hash: &[u8; 32],
    between: &mut dyn FnMut(),
) -> OriginalWipe {
    let Some((mut file, restore)) = open_original(path, listed, len) else {
        return OriginalWipe::Kept;
    };
    let hashed = crypto::hash_reader(&mut file).is_ok_and(|h| h == *hash);
    if hashed {
        between();
    }
    // Unix cannot keep other writers out: refuse if the file changed in the meantime.
    let ready = hashed && is_as_listed(&file, listed, len) && file.seek(SeekFrom::Start(0)).is_ok();
    if !ready {
        if let Some(permissions) = restore {
            let _ = file.set_permissions(permissions);
        }
        return OriginalWipe::Kept;
    }
    if write_zeros(&mut file, len).is_err()
        || !file.metadata().is_ok_and(|m| m.len() == len) // grown meanwhile: keep the rest
        || file.set_len(0).and_then(|()| file.sync_all()).is_err()
    {
        return OriginalWipe::Kept;
    }
    let Ok(emptied) = file.metadata() else {
        return OriginalWipe::Kept;
    };
    drop(file);
    dispose(path, &emptied)
}

/// Opens the original at `path` exclusively if it is the plain, singly linked file `listed`
/// describes, clearing the read-only attribute if needed (returned for restoring).
fn open_original(
    path: &Path,
    listed: Baseline,
    len: u64,
) -> Option<(File, Option<fs::Permissions>)> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || has_other_names(&meta) || stat(&meta) != listed || meta.len() != len {
        return None;
    }
    let (file, restore) = match open_exclusive(path) {
        Ok(file) => (file, None),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied && meta.permissions().readonly() => {
            // Read-only: clear the attribute through a handle, so a swapped-in link is not
            // followed.
            let handle = still_the_same(open_no_follow(path, false).ok()?, &meta).ok()??;
            handle.set_permissions(writable(meta.permissions())).ok()?;
            drop(handle);
            let Ok(file) = open_exclusive(path) else {
                restore_permissions(path, &meta); // open elsewhere, say
                return None;
            };
            (file, Some(meta.permissions()))
        }
        Err(_) => return None,
    };
    match still_the_same(file, &meta) {
        Ok(Some(file)) if is_as_listed(&file, listed, len) => Some((file, restore)),
        Ok(Some(file)) => {
            if let Some(permissions) = restore {
                let _ = file.set_permissions(permissions);
            }
            None
        }
        _ => {
            if restore.is_some() {
                restore_permissions(path, &meta);
            }
            None
        }
    }
}

/// Puts the read-only attribute back on the file `meta` describes, through a handle that does
/// not follow a link (best effort; nothing is done if something else is at `path` now).
fn restore_permissions(path: &Path, meta: &fs::Metadata) {
    if let Ok(handle) = open_no_follow(path, false)
        && let Ok(Some(handle)) = still_the_same(handle, meta)
    {
        let _ = handle.set_permissions(meta.permissions());
    }
}

/// `true` when the open file still has the `listed` length and time and no other name.
fn is_as_listed(file: &File, listed: Baseline, len: u64) -> bool {
    file.metadata()
        .is_ok_and(|m| !has_other_names(&m) && stat(&m) == listed && m.len() == len)
}

/// Opens a file for reading and writing, exclusively and without following a reparse point on
/// Windows. Unix has no mandatory exclusion; [`still_the_same`] catches a followed link.
fn open_exclusive(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

/// Removes the emptied original: renamed to a random name first, deleted only if what moved is
/// still the emptied file (`emptied`: its metadata after truncation), else put back.
fn dispose(path: &Path, emptied: &fs::Metadata) -> OriginalWipe {
    let (Some(parent), Ok(random)) = (path.parent(), crypto::random::<8>()) else {
        return OriginalWipe::Kept;
    };
    let moved = parent.join(hex(&random));
    if fs::rename(path, &moved).is_err() {
        return OriginalWipe::Kept; // an empty file stays; it is imported and wiped next time
    }
    if is_emptied(&moved, emptied) {
        return if fs::remove_file(&moved).is_ok() {
            OriginalWipe::Wiped
        } else {
            OriginalWipe::Kept
        };
    }
    // Something else had replaced the original (which is then already gone). A plain file goes
    // back under its name by hard link, which never replaces whatever may be at `path` now (and
    // is never asked to follow a link). Anything else, or a file that cannot be linked back
    // (FAT32, exFAT, the name taken again), is set aside as `<name>.restored-<hex>`.
    let plain = fs::symlink_metadata(&moved).is_ok_and(|m| m.is_file());
    if plain && fs::hard_link(&moved, path).is_ok() {
        let _ = fs::remove_file(&moved);
        return OriginalWipe::Wiped;
    }
    let mut aside = path.as_os_str().to_owned();
    aside.push(".restored-");
    aside.push(hex(&random));
    let _ = fs::rename(&moved, PathBuf::from(aside));
    OriginalWipe::Kept
}

/// `true` when `path` is a plain, empty, singly linked file (on Unix, the very file `emptied`
/// describes).
fn is_emptied(path: &Path, emptied: &fs::Metadata) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    let same = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            (meta.dev(), meta.ino()) == (emptied.dev(), emptied.ino())
        }
        #[cfg(not(unix))]
        {
            let _ = emptied;
            true
        }
    };
    same && meta.is_file() && meta.len() == 0 && !has_other_names(&meta)
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

    /// A synthetic original in its own folder: `(folder, path, listed, hash)`.
    fn original(bytes: &[u8]) -> io::Result<(tempfile::TempDir, PathBuf, Baseline, [u8; 32])> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("original.txt");
        fs::write(&path, bytes)?;
        let listed = stat(&fs::symlink_metadata(&path)?);
        let hash = crypto::hash_reader(&mut File::open(&path)?)?;
        Ok((dir, path, listed, hash))
    }

    fn names_in(dir: &Path) -> io::Result<Vec<String>> {
        let mut names: Vec<String> = fs::read_dir(dir)?
            .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect::<io::Result<_>>()?;
        names.sort();
        Ok(names)
    }

    #[test]
    fn an_unchanged_original_is_wiped_and_removed() -> io::Result<()> {
        let (dir, path, listed, hash) = original(b"plain original")?;
        assert_eq!(wipe_original(&path, listed, 14, &hash), OriginalWipe::Wiped);
        assert_eq!(names_in(dir.path())?, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn an_original_replaced_by_rename_is_kept() -> io::Result<()> {
        let (dir, path, listed, hash) = original(b"plain original")?;
        // An atomic save: same length and time, other content, moved over the name.
        let staged = dir.path().join("staged.txt");
        fs::write(&staged, b"NEW CONTENT!!!")?;
        if let Some(time) = listed.1 {
            File::options()
                .write(true)
                .open(&staged)?
                .set_modified(time)?;
        }
        fs::rename(&staged, &path)?;
        assert_eq!(stat(&fs::symlink_metadata(&path)?), listed);
        assert_eq!(wipe_original(&path, listed, 14, &hash), OriginalWipe::Kept);
        assert_eq!(fs::read(&path)?, b"NEW CONTENT!!!");
        assert_eq!(names_in(dir.path())?, vec!["original.txt".to_owned()]);
        Ok(())
    }

    #[test]
    fn a_replacement_made_after_the_hash_is_never_wiped() -> io::Result<()> {
        let (dir, path, listed, hash) = original(b"plain original")?;
        let staged = dir.path().join("staged.txt");
        let mut swapped = false;
        let mut swap = || {
            // Refused on Windows, where the original is held open exclusively.
            swapped = fs::write(&staged, b"saved meanwhile")
                .and_then(|()| fs::rename(&staged, &path))
                .is_ok();
        };
        let result = wipe_original_with(&path, listed, 14, &hash, &mut swap);
        // Either way the original itself is gone.
        assert_eq!(result, OriginalWipe::Wiped);
        if swapped {
            assert_eq!(names_in(dir.path())?, vec!["original.txt".to_owned()]);
            assert_eq!(fs::read(&path)?, b"saved meanwhile");
        } else {
            assert_eq!(names_in(dir.path())?, vec!["staged.txt".to_owned()]);
        }
        Ok(())
    }

    #[test]
    fn a_write_after_the_hash_is_not_lost() -> io::Result<()> {
        let (dir, path, listed, hash) = original(b"plain original")?;
        let mut appended = false;
        let mut append = || {
            // Refused on Windows (exclusive handle); lands in the same file elsewhere.
            appended = File::options()
                .append(true)
                .open(&path)
                .and_then(|mut file| file.write_all(b"+appended"))
                .is_ok();
        };
        let result = wipe_original_with(&path, listed, 14, &hash, &mut append);
        if appended {
            assert_eq!(result, OriginalWipe::Kept);
            assert_eq!(fs::read(&path)?, b"plain original+appended");
            assert_eq!(names_in(dir.path())?, vec!["original.txt".to_owned()]);
        } else {
            assert_eq!(result, OriginalWipe::Wiped);
            assert_eq!(names_in(dir.path())?, Vec::<String>::new());
        }
        Ok(())
    }

    #[test]
    fn a_changed_original_is_kept_untouched() -> io::Result<()> {
        let (_dir, path, listed, hash) = original(b"plain original")?;
        let other = crypto::hash_reader(&mut &b"something else"[..])?;
        assert_eq!(wipe_original(&path, listed, 14, &other), OriginalWipe::Kept);
        assert_eq!(wipe_original(&path, listed, 13, &hash), OriginalWipe::Kept);
        assert_eq!(fs::read(&path)?, b"plain original");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_link_swapped_in_after_the_hash_is_set_aside_not_put_back() -> io::Result<()> {
        let (dir, path, listed, hash) = original(b"plain original")?;
        let outside = tempfile::tempdir()?;
        let target = outside.path().join("target.txt");
        fs::write(&target, b"OUTSIDE")?;
        let staged = dir.path().join("staged.txt");
        let mut swap = || {
            let _ = std::os::unix::fs::symlink(&target, &staged)
                .and_then(|()| fs::rename(&staged, &path));
        };
        let result = wipe_original_with(&path, listed, 14, &hash, &mut swap);
        assert_eq!(result, OriginalWipe::Kept);
        let names = names_in(dir.path())?;
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names[0].starts_with("original.txt.restored-"), "{names:?}");
        let kept = fs::symlink_metadata(dir.path().join(&names[0]))?;
        assert!(kept.file_type().is_symlink());
        assert_eq!(fs::read(&target)?, b"OUTSIDE");
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn a_read_only_original_open_elsewhere_keeps_its_attribute() -> io::Result<()> {
        let (_dir, path, _, hash) = original(b"plain original")?;
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions)?;
        let listed = stat(&fs::symlink_metadata(&path)?);
        let held = File::open(&path)?; // the exclusive open fails while this is open
        assert_eq!(wipe_original(&path, listed, 14, &hash), OriginalWipe::Kept);
        drop(held);
        assert!(fs::metadata(&path)?.permissions().readonly());
        assert_eq!(fs::read(&path)?, b"plain original");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn linked_originals_are_kept() -> io::Result<()> {
        let (dir, path, listed, hash) = original(b"plain original")?;
        let other_name = dir.path().join("other-name.txt");
        fs::hard_link(&path, &other_name)?;
        let listed_now = stat(&fs::symlink_metadata(&path)?);
        assert_eq!(
            wipe_original(&path, listed_now, 14, &hash),
            OriginalWipe::Kept
        );
        assert_eq!(fs::read(&other_name)?, b"plain original");
        fs::remove_file(&other_name)?;

        let outside = tempfile::tempdir()?;
        let target = outside.path().join("target.txt");
        fs::rename(&path, &target)?;
        std::os::unix::fs::symlink(&target, &path)?;
        assert_eq!(wipe_original(&path, listed, 14, &hash), OriginalWipe::Kept);
        assert_eq!(fs::read(&target)?, b"plain original");
        assert!(fs::symlink_metadata(&path)?.file_type().is_symlink());
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

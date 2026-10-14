//! Namespace operations: list, `create_dir`, rename, delete. Each is one index commit.

// cspell:ignore rfind

use std::fs;
use std::time::{Duration, UNIX_EPOCH};

use super::{Inner, Unlocked, Vault, now_ms};
use crate::error::VaultError;
use crate::format::index::{Entry, Index, MAX_ENTRIES};
use crate::path::VaultPath;
use crate::session;
use crate::{EntryInfo, EntryKind};

/// The unlocked state, or `Locked` / `Uninitialized`.
pub(super) fn unlocked_mut(inner: &mut Inner) -> Result<&mut Unlocked, VaultError> {
    let error = if inner.exists {
        VaultError::Locked
    } else {
        VaultError::Uninitialized
    };
    inner.unlocked.as_mut().ok_or(error)
}

/// `Ok` when `dir` is the root or an existing folder.
pub(super) fn require_dir(index: &Index, dir: &VaultPath) -> Result<(), VaultError> {
    if dir.is_root() {
        return Ok(());
    }
    match index.entries.get(dir) {
        Some(entry) if entry.kind == EntryKind::Dir => Ok(()),
        _ => Err(VaultError::NotFound(dir.as_str().to_owned())),
    }
}

/// The child of `dir` whose name equals `name` ignoring case, if any.
pub(super) fn find_sibling(index: &Index, dir: &VaultPath, name: &str) -> Option<VaultPath> {
    let folded = name.to_lowercase();
    let depth = dir.depth() + 1;
    index
        .entries
        .keys()
        .find(|p| {
            p.depth() == depth
                && p.starts_with(dir)
                && p.file_name().is_some_and(|n| n.to_lowercase() == folded)
        })
        .cloned()
}

/// `name` split at its last period (a leading period starts no extension).
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => name.split_at(i),
        _ => (name, ""),
    }
}

/// The first free `"<stem> (n)<ext>"` in `dir`, for [`crate::Conflict::KeepBoth`].
pub(super) fn keep_both_name(
    index: &Index,
    dir: &VaultPath,
    name: &str,
) -> Result<VaultPath, VaultError> {
    let (stem, ext) = split_extension(name);
    for n in 2..10_000 {
        let candidate = format!("{stem} ({n}){ext}");
        let path = dir.join(&candidate)?;
        if find_sibling(index, dir, &candidate).is_none() {
            return Ok(path);
        }
    }
    Err(VaultError::LimitExceeded("too many copies of one name"))
}

/// `LimitExceeded` when adding `adding` entries would pass the cap.
pub(super) fn check_capacity(index: &Index, adding: usize) -> Result<(), VaultError> {
    if index.entries.len() + adding > MAX_ENTRIES {
        return Err(VaultError::LimitExceeded(
            "a vault holds at most 8192 files and folders",
        ));
    }
    Ok(())
}

pub(super) fn info(path: &VaultPath, entry: &Entry) -> EntryInfo {
    EntryInfo {
        path: path.clone(),
        kind: entry.kind,
        size: entry.size,
        modified: UNIX_EPOCH + Duration::from_millis(entry.modified_ms),
    }
}

impl Vault {
    /// The direct children of `dir` (`VaultPath::root()` for the top level).
    pub fn list(&self, dir: &VaultPath) -> Result<Vec<EntryInfo>, VaultError> {
        let mut inner = self.inner()?;
        let unlocked = unlocked_mut(&mut inner)?;
        require_dir(&unlocked.index, dir)?;
        let depth = dir.depth() + 1;
        Ok(unlocked
            .index
            .entries
            .iter()
            .filter(|(p, _)| p.depth() == depth && p.starts_with(dir))
            .map(|(p, e)| info(p, e))
            .collect())
    }

    /// Creates one folder; its parent must exist.
    pub fn create_dir(&self, path: &VaultPath) -> Result<(), VaultError> {
        let mut inner = self.inner()?;
        let unlocked = unlocked_mut(&mut inner)?;
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(VaultError::Exists(String::new()));
        };
        require_dir(&unlocked.index, &parent)?;
        if find_sibling(&unlocked.index, &parent, name).is_some() {
            return Err(VaultError::Exists(path.as_str().to_owned()));
        }
        check_capacity(&unlocked.index, 1)?;
        let mut next = unlocked.index.clone();
        next.entries.insert(path.clone(), Entry::dir(now_ms()));
        self.commit(unlocked, next)
    }

    /// Renames or moves a file or folder (with everything inside) in one index commit.
    pub fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<(), VaultError> {
        let mut inner = self.inner()?;
        let unlocked = unlocked_mut(&mut inner)?;
        if from.is_root() || to.is_root() {
            return Err(VaultError::InvalidPath("the vault root cannot be renamed"));
        }
        if !unlocked.index.entries.contains_key(from) {
            return Err(VaultError::NotFound(from.as_str().to_owned()));
        }
        if from == to {
            return Ok(());
        }
        if to.starts_with(from) {
            return Err(VaultError::InvalidPath(
                "a folder cannot be moved into itself",
            ));
        }
        let (Some(parent), Some(name)) = (to.parent(), to.file_name()) else {
            return Err(VaultError::InvalidPath("the vault root cannot be renamed"));
        };
        require_dir(&unlocked.index, &parent)?;
        if find_sibling(&unlocked.index, &parent, name).is_some_and(|existing| existing != *from) {
            return Err(VaultError::Exists(to.as_str().to_owned()));
        }
        let mut next = unlocked.index.clone();
        let moved: Vec<VaultPath> = next
            .entries
            .keys()
            .filter(|p| p.starts_with(from))
            .cloned()
            .collect();
        for old in moved {
            let new = old.rebase(from, to)?;
            if let Some(entry) = next.entries.remove(&old) {
                next.entries.insert(new, entry);
            }
        }
        self.commit(unlocked, next)?;
        for file in &mut unlocked.session.files {
            if file.entry.starts_with(from)
                && let Ok(new) = file.entry.rebase(from, to)
            {
                file.entry = new;
            }
        }
        Ok(())
    }

    /// Deletes a file, or a folder and everything inside it. Removes the blobs and wipes any
    /// open session copies; the ciphertext is deleted, not overwritten (research/vault.md 5).
    pub fn delete(&self, path: &VaultPath) -> Result<(), VaultError> {
        let mut inner = self.inner()?;
        let unlocked = unlocked_mut(&mut inner)?;
        if path.is_root() {
            return Err(VaultError::InvalidPath("the vault root cannot be deleted"));
        }
        if !unlocked.index.entries.contains_key(path) {
            return Err(VaultError::NotFound(path.as_str().to_owned()));
        }
        let mut next = unlocked.index.clone();
        let doomed: Vec<VaultPath> = next
            .entries
            .keys()
            .filter(|p| p.starts_with(path))
            .cloned()
            .collect();
        let blobs: Vec<[u8; 16]> = doomed
            .iter()
            .filter_map(|p| next.entries.remove(p))
            .filter_map(|e| e.blob)
            .collect();
        self.commit(unlocked, next)?;
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut unlocked.session.files)
            .into_iter()
            .partition(|f| f.entry.starts_with(path));
        unlocked.session.files = kept;
        drop(inner);
        for file in &gone {
            session::wipe_session_file(file);
        }
        for blob in blobs {
            let _ = fs::remove_file(self.layout.blob(&blob));
        }
        Ok(())
    }
}

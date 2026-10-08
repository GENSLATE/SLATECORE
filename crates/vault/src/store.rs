//! The vault folder on disk: slots, commits, blobs, garbage collection, the instance lock and
//! the foreign-item scan (research/vault.md 4.1, 5, 6.7, 6.8).
//!
//! Crash safety does not rely on rename-over-existing (not atomic on FAT32/exFAT): blobs are
//! written once with `create_new`, and the header and index each have two generation-numbered
//! slots that are overwritten in place and synced. The highest valid generation wins, and a
//! commit always goes to the slot that does not hold it. The only rename is the first header
//! slot at `create`, onto a name that does not exist yet.

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::crypto::{self, SecretKey};
use crate::error::VaultError;
use crate::format::header::{HEADER_LEN, HeaderSlot};
use crate::format::index::{
    INDEX_HEADER_LEN, Index, MAX_INDEX_PLAINTEXT, blob_name, parse_blob_name,
};

const OWNED_NAMES: [&str; 8] = [
    "vault.a.gvh",
    "vault.b.gvh",
    "vault.a.gvh.tmp",
    "index.a.gvi",
    "index.b.gvi",
    "vault.guard",
    "vault.lock",
    "files",
];

/// Paths inside `storage/vault`.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub(crate) root: PathBuf,
    pub(crate) files: PathBuf,
    pub(crate) headers: [PathBuf; 2],
    /// Where `create` stages slot A before renaming it into place.
    pub(crate) header_temp: PathBuf,
    pub(crate) indexes: [PathBuf; 2],
    pub(crate) guard: PathBuf,
    pub(crate) lock: PathBuf,
}

impl Layout {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            files: root.join("files"),
            headers: [root.join("vault.a.gvh"), root.join("vault.b.gvh")],
            header_temp: root.join("vault.a.gvh.tmp"),
            indexes: [root.join("index.a.gvi"), root.join("index.b.gvi")],
            guard: root.join("vault.guard"),
            lock: root.join("vault.lock"),
        }
    }

    pub(crate) fn blob(&self, id: &[u8; 16]) -> PathBuf {
        self.files.join(format!("{}.gvf", blob_name(id)))
    }
}

/// A slot file as found on disk.
enum SlotFile {
    Missing,
    /// Present but not readable as a file (an I/O error, or a folder in its place). Treated
    /// like a damaged slot so the other slot can still be used.
    Unreadable(std::io::Error),
    Bytes(Vec<u8>),
}

/// Reads a whole slot file, at most `max` bytes (one more is read to detect an oversized file).
fn read_slot(path: &Path, max: usize) -> SlotFile {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return SlotFile::Missing,
        Err(e) => return SlotFile::Unreadable(e),
    };
    let mut bytes = Vec::new();
    match file
        .take(u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
    {
        Ok(_) => SlotFile::Bytes(bytes),
        Err(e) => SlotFile::Unreadable(e),
    }
}

/// Overwrites a slot file in place and syncs it. The return of `sync_all` is the commit point.
pub(crate) fn write_slot(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(VaultError::io("writing a vault slot"))?;
    file.write_all(bytes)
        .map_err(VaultError::io("writing a vault slot"))?;
    file.sync_all()
        .map_err(VaultError::io("syncing a vault slot"))
}

/// Both header slots as found on disk.
pub(crate) struct HeaderRead {
    /// `true` when at least one slot file exists (the vault has been created).
    pub(crate) exists: bool,
    /// The slot with the highest valid generation, or why none is usable.
    pub(crate) winner: Result<HeaderSlot, VaultError>,
    /// Slots that are missing, invalid or older than the winner.
    pub(crate) stale: [bool; 2],
}

/// Parses both header slots and picks the highest valid generation (research/vault.md 4.2).
/// A slot that cannot be read counts as damaged.
pub(crate) fn read_headers(layout: &Layout) -> HeaderRead {
    let mut exists = false;
    let mut parsed: [Option<HeaderSlot>; 2] = [None, None];
    let mut newer_format = None;
    let mut io_error = None;
    for (i, path) in layout.headers.iter().enumerate() {
        let bytes = match read_slot(path, HEADER_LEN) {
            SlotFile::Missing => continue,
            SlotFile::Unreadable(e) => {
                exists = true;
                io_error.get_or_insert(e);
                continue;
            }
            SlotFile::Bytes(bytes) => bytes,
        };
        exists = true;
        match HeaderSlot::decode(&bytes) {
            Ok(slot) => parsed[i] = Some(slot),
            // A checksum-valid slot from a newer format is worth reporting over "damaged".
            Err(e @ (VaultError::UnsupportedVersion(_) | VaultError::UnsupportedKdf)) => {
                newer_format = Some(e);
            }
            Err(_) => {}
        }
    }
    let winner = match (&parsed[0], &parsed[1]) {
        (Some(a), Some(b)) => Ok(if b.generation > a.generation {
            b.clone()
        } else {
            a.clone()
        }),
        (Some(a), None) => Ok(a.clone()),
        (None, Some(b)) => Ok(b.clone()),
        (None, None) => Err(newer_format
            .or_else(|| {
                io_error.map(|source| VaultError::Io {
                    context: "reading the vault header",
                    source,
                })
            })
            .unwrap_or(VaultError::HeaderDamaged)),
    };
    let stale = match &winner {
        Ok(w) => [0, 1].map(|i| parsed[i].as_ref().is_none_or(|s| *s != *w)),
        Err(_) => [false, false],
    };
    HeaderRead {
        exists,
        winner,
        stale,
    }
}

/// Writes a header to slot A, syncs, then slot B, syncs (identical bytes).
pub(crate) fn write_headers(layout: &Layout, slot: &HeaderSlot) -> Result<(), VaultError> {
    let bytes = slot.encode();
    write_slot(&layout.headers[0], &bytes)?;
    write_slot(&layout.headers[1], &bytes)
}

/// The first header of a new vault. Slot A is written to a temporary file and renamed into
/// place, so a crash leaves either no header (the vault can be created again) or a complete
/// slot A; slot B follows in place.
pub(crate) fn write_first_headers(layout: &Layout, slot: &HeaderSlot) -> Result<(), VaultError> {
    let bytes = slot.encode();
    write_slot(&layout.header_temp, &bytes)?;
    fs::rename(&layout.header_temp, &layout.headers[0])
        .map_err(VaultError::io("creating the vault header"))?;
    sync_dir(&layout.root);
    write_slot(&layout.headers[1], &bytes)
}

/// Removes what an interrupted `create` may have left: index slots and a staged header.
pub(crate) fn remove_create_leftovers(layout: &Layout) -> Result<(), VaultError> {
    for path in layout.indexes.iter().chain([&layout.header_temp]) {
        match fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(VaultError::Io {
                    context: "creating the vault",
                    source: e,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Best effort: makes a rename in `dir` durable where the platform allows opening a folder.
fn sync_dir(dir: &Path) {
    if let Ok(handle) = File::open(dir) {
        let _ = handle.sync_all();
    }
}

/// Rewrites the stale header slots from the winner (best effort, after a successful unlock).
pub(crate) fn heal_headers(layout: &Layout, winner: &HeaderSlot, stale: [bool; 2]) {
    let bytes = winner.encode();
    for (i, is_stale) in stale.into_iter().enumerate() {
        if is_stale {
            let _ = write_slot(&layout.headers[i], &bytes);
        }
    }
}

/// The index as loaded at unlock.
pub(crate) struct IndexRead {
    pub(crate) generation: u64,
    pub(crate) index: Index,
    /// The slot (0 = A, 1 = B) holding `generation`; the next commit goes to the other one.
    pub(crate) slot: usize,
}

/// Decrypts both index slots, uses the highest valid generation and rewrites a missing or
/// broken slot from it. Neither slot valid: [`VaultError::Tampered`]`("index")`.
pub(crate) fn load_index(
    layout: &Layout,
    key: &SecretKey,
    vault_id: &[u8; 16],
) -> Result<IndexRead, VaultError> {
    let max = INDEX_HEADER_LEN + MAX_INDEX_PLAINTEXT + 16;
    let mut slots: [Option<(u64, Index, Vec<u8>)>; 2] = [None, None];
    for (i, path) in layout.indexes.iter().enumerate() {
        if let SlotFile::Bytes(bytes) = read_slot(path, max)
            && let Ok((generation, index)) = crypto::open_index(key, vault_id, &bytes)
        {
            slots[i] = Some((generation, index, bytes));
        }
    }
    let [a, b] = slots;
    let (winner, slot, other_valid) = match (a, b) {
        (Some(a), Some(b)) if b.0 > a.0 => (b, 1, true),
        (Some(a), Some(_)) => (a, 0, true),
        (Some(a), None) => (a, 0, false),
        (None, Some(b)) => (b, 1, false),
        (None, None) => return Err(VaultError::Tampered(crate::error::tamper::INDEX)),
    };
    let (generation, index, bytes) = winner;
    if !other_valid {
        let _ = write_slot(&layout.indexes[1 - slot], &bytes);
    }
    Ok(IndexRead {
        generation,
        index,
        slot,
    })
}

/// Commits `index` as generation `generation` into `slot`, which must be the slot *not*
/// holding the current generation, so a torn write always leaves the previous one intact.
pub(crate) fn commit_index(
    layout: &Layout,
    key: &SecretKey,
    vault_id: &[u8; 16],
    generation: u64,
    index: &Index,
    slot: usize,
) -> Result<(), VaultError> {
    let sealed = crypto::seal_index(key, vault_id, generation, index)?;
    let path = layout
        .indexes
        .get(slot)
        .ok_or(VaultError::Internal("index slot out of range"))?;
    write_slot(path, &sealed)
}

/// Every well-formed blob id currently in `files/`.
pub(crate) fn list_blobs(layout: &Layout) -> Vec<[u8; 16]> {
    let Ok(entries) = fs::read_dir(&layout.files) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|e| blob_id_of(&e.file_name()))
        .collect()
}

fn blob_id_of(name: &OsStr) -> Option<[u8; 16]> {
    let name = name.to_str()?;
    parse_blob_name(name.strip_suffix(".gvf")?)
}

/// Deletes every blob the index does not reference. Never touches other files in `files/`.
pub(crate) fn collect_garbage(layout: &Layout, index: &Index) -> usize {
    let referenced: std::collections::HashSet<[u8; 16]> =
        index.entries.values().filter_map(|e| e.blob).collect();
    let mut removed = 0;
    for id in list_blobs(layout) {
        if !referenced.contains(&id) && fs::remove_file(layout.blob(&id)).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Creates a new, empty blob file with a fresh random id (`create_new`; retries on collision).
pub(crate) fn create_blob(layout: &Layout) -> Result<([u8; 16], File), VaultError> {
    fs::create_dir_all(&layout.files).map_err(VaultError::io("creating the files folder"))?;
    for _ in 0..8 {
        let id: [u8; 16] = crypto::random()?;
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(layout.blob(&id))
        {
            Ok(file) => return Ok((id, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(VaultError::Io {
                    context: "creating an encrypted file",
                    source: e,
                });
            }
        }
    }
    Err(VaultError::Internal("could not pick a free file id"))
}

/// Takes the exclusive instance lock on `vault.lock` (held for the life of the handle).
pub(crate) fn take_instance_lock(layout: &Layout) -> Result<File, VaultError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&layout.lock)
        .map_err(VaultError::io("opening vault.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(VaultError::Busy),
        Err(TryLockError::Error(e)) => Err(VaultError::Io {
            context: "locking vault.lock",
            source: e,
        }),
    }
}

/// Entries directly inside the vault root that the vault does not own (plain files dropped in
/// with Explorer, for example).
pub(crate) fn foreign_items(layout: &Layout) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(&layout.root) else {
        return Vec::new();
    };
    let mut items: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| {
            !e.file_name()
                .to_str()
                .is_some_and(|n| OWNED_NAMES.contains(&n))
        })
        .map(|e| e.path())
        .collect();
    items.sort();
    items
}

//! Streaming transfers: import, export, open in the session folder, verify, foreign items.
//!
//! Each transfer takes a [`Ctx`] under the state lock, streams without it (checking the epoch
//! every chunk), and re-takes the lock only to commit. Plaintext is never handed out before the
//! last chunk has authenticated: partial outputs are wiped.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::entries::{check_capacity, find_sibling, info, keep_both_name, require_dir};
use super::{Ctx, Vault, now_ms};
use crate::crypto::{self, StreamSummary};
use crate::error::{VaultError, tamper};
use crate::format::blob::{BlobHeader, DEFAULT_CHUNK_SIZE, encrypted_len};
use crate::format::index::Entry;
use crate::path::{VaultPath, validate_component};
use crate::session::{self, Baseline, SessionFile, WipeReport};
use crate::store;
use crate::{Conflict, EntryInfo, EntryKind, Problem, ProgressFn, VerifyReport};

/// The validated vault name for an OS path's last component.
pub(super) fn source_name(path: &Path) -> Result<String, VaultError> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(VaultError::InvalidPath(
            "the file name is not valid Unicode",
        ))?;
    validate_component(name)?;
    Ok(name.to_owned())
}

fn ms_of(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// `<dest>.gvpart`, the export staging name.
fn part_path(dest: &Path) -> PathBuf {
    let mut name: OsString = dest.as_os_str().to_owned();
    name.push(".gvpart");
    PathBuf::from(name)
}

fn wipe_quietly(path: &Path) {
    let mut report = WipeReport::default();
    session::wipe_path(path, &mut report);
}

impl Vault {
    /// Encrypts `reader` into a new blob; on any failure the partial blob is deleted.
    pub(super) fn write_blob(
        &self,
        ctx: &Ctx,
        reader: &mut dyn Read,
        total: u64,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<([u8; 16], StreamSummary), VaultError> {
        let (id, mut file) = store::create_blob(&self.layout)?;
        let written = (|| {
            let header = BlobHeader {
                chunk_size: DEFAULT_CHUNK_SIZE,
                blob_id: id,
                stream_nonce: crypto::random()?,
            };
            let mut hook = |done: u64| {
                self.check(ctx)?;
                if let Some(report) = progress {
                    report(done, total);
                }
                Ok(())
            };
            let summary = crypto::encrypt_stream(
                &ctx.key,
                &ctx.vault_id,
                &header,
                reader,
                &mut file,
                self.config.max_blob_bytes,
                &mut hook,
            )?;
            file.sync_all()
                .map_err(VaultError::io("syncing an encrypted file"))?;
            Ok(summary)
        })();
        drop(file);
        match written {
            Ok(summary) => Ok((id, summary)),
            Err(e) => {
                let _ = fs::remove_file(self.layout.blob(&id));
                Err(e)
            }
        }
    }

    /// Decrypts blob `id` (expected plaintext `size`) into `out`.
    fn read_blob(
        &self,
        ctx: &Ctx,
        id: &[u8; 16],
        size: u64,
        out: &mut dyn std::io::Write,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<StreamSummary, VaultError> {
        let mut file = match File::open(self.layout.blob(id)) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(VaultError::BlobMissing);
            }
            Err(e) => {
                return Err(VaultError::Io {
                    context: "opening an encrypted file",
                    source: e,
                });
            }
        };
        let len = file
            .metadata()
            .map_err(VaultError::io("reading an encrypted file"))?
            .len();
        let mut hook = |done: u64| {
            self.check(ctx)?;
            if let Some(report) = progress {
                report(done, size);
            }
            Ok(())
        };
        crypto::decrypt_stream(
            &ctx.key,
            &ctx.vault_id,
            id,
            &mut file,
            len,
            Some(size),
            out,
            &mut hook,
        )
    }

    /// The file entry at `path` (folders are not files).
    fn file_entry(&self, ctx: &Ctx, path: &VaultPath) -> Result<Entry, VaultError> {
        let mut inner = self.inner()?;
        let unlocked = self.current(&mut inner, ctx)?;
        match unlocked.index.entries.get(path) {
            Some(entry) if entry.kind == EntryKind::File => Ok(entry.clone()),
            Some(_) => Err(VaultError::InvalidPath("not a file")),
            None => Err(VaultError::NotFound(path.as_str().to_owned())),
        }
    }

    /// Encrypts a file or a folder (recursively) from outside the vault into `dest_dir`.
    /// The source is left in place.
    pub fn import(
        &self,
        source: &Path,
        dest_dir: &VaultPath,
        on_conflict: Conflict,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<EntryInfo, VaultError> {
        let ctx = self.ctx()?;
        let name = source_name(source)?;
        let meta = fs::metadata(source).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => VaultError::NotFound(source.display().to_string()),
            _ => VaultError::Io {
                context: "reading the file to import",
                source: e,
            },
        })?;
        if meta.is_dir() {
            self.import_dir(&ctx, source, dest_dir, &name, on_conflict, progress)
        } else {
            self.import_file(&ctx, source, &meta, dest_dir, &name, on_conflict, progress)
        }
    }

    #[allow(clippy::too_many_arguments)] // internal; mirrors `import` plus the resolved pieces
    fn import_file(
        &self,
        ctx: &Ctx,
        source: &Path,
        meta: &fs::Metadata,
        dest_dir: &VaultPath,
        name: &str,
        conflict: Conflict,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<EntryInfo, VaultError> {
        {
            // Cheap early refusals; everything is checked again at commit.
            let mut inner = self.inner()?;
            let unlocked = self.current(&mut inner, ctx)?;
            require_dir(&unlocked.index, dest_dir)?;
            if conflict == Conflict::Fail && find_sibling(&unlocked.index, dest_dir, name).is_some()
            {
                return Err(VaultError::Exists(dest_dir.join(name)?.as_str().to_owned()));
            }
        }
        if let Some(max) = self.config.max_blob_bytes
            && encrypted_len(meta.len(), DEFAULT_CHUNK_SIZE) > max
        {
            return Err(VaultError::FileTooLarge);
        }
        let modified_ms = meta.modified().map_or_else(|_| now_ms(), ms_of);
        let mut file = File::open(source).map_err(VaultError::io("opening the file to import"))?;
        let (blob, summary) = self.write_blob(ctx, &mut file, meta.len(), progress)?;
        let entry = Entry::file(blob, summary.plain_len, modified_ms);
        let committed = self.commit_new_entry(ctx, dest_dir, name, conflict, entry);
        if committed.is_err() {
            let _ = fs::remove_file(self.layout.blob(&blob));
        }
        committed
    }

    /// Adds a file or folder entry named `name` to `dest_dir`, resolving a name conflict.
    /// A folder that meets an existing folder under `Replace` merges into it.
    fn commit_new_entry(
        &self,
        ctx: &Ctx,
        dest_dir: &VaultPath,
        name: &str,
        conflict: Conflict,
        entry: Entry,
    ) -> Result<EntryInfo, VaultError> {
        let mut inner = self.inner()?;
        let unlocked = self.current(&mut inner, ctx)?;
        require_dir(&unlocked.index, dest_dir)?;
        let target = dest_dir.join(name)?;
        let existing = find_sibling(&unlocked.index, dest_dir, name)
            .and_then(|p| unlocked.index.entries.get(&p).map(|e| (p, e.kind, e.blob)));
        let (path, superseded) = match (existing, conflict) {
            (None, _) => (target, None),
            (Some((path, EntryKind::Dir, _)), Conflict::Replace)
                if entry.kind == EntryKind::Dir =>
            {
                let found = unlocked.index.entries.get(&path).map(|e| info(&path, e));
                return found.ok_or(VaultError::Internal("folder vanished"));
            }
            (Some((path, EntryKind::File, old)), Conflict::Replace)
                if entry.kind == EntryKind::File =>
            {
                (path, old)
            }
            (Some(_), Conflict::KeepBoth) => {
                (keep_both_name(&unlocked.index, dest_dir, name)?, None)
            }
            (Some(_), _) => return Err(VaultError::Exists(target.as_str().to_owned())),
        };
        if superseded.is_none() {
            check_capacity(&unlocked.index, 1)?;
        }
        let listed = info(&path, &entry);
        let mut next = unlocked.index.clone();
        next.entries.insert(path, entry);
        self.commit(unlocked, next)?;
        drop(inner);
        if let Some(old) = superseded {
            let _ = fs::remove_file(self.layout.blob(&old));
        }
        Ok(listed)
    }

    fn import_dir(
        &self,
        ctx: &Ctx,
        source: &Path,
        dest_dir: &VaultPath,
        name: &str,
        conflict: Conflict,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<EntryInfo, VaultError> {
        let folder = self.commit_new_entry(ctx, dest_dir, name, conflict, Entry::dir(now_ms()))?;
        let mut children: Vec<PathBuf> = fs::read_dir(source)
            .map_err(VaultError::io("reading the folder to import"))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        children.sort();
        for child in children {
            let meta = fs::symlink_metadata(&child)
                .map_err(VaultError::io("reading the folder to import"))?;
            if meta.file_type().is_symlink() {
                continue; // never follow links out of the folder being imported
            }
            let child_name = source_name(&child)?;
            if meta.is_dir() {
                self.import_dir(ctx, &child, &folder.path, &child_name, conflict, progress)?;
            } else {
                self.import_file(
                    ctx,
                    &child,
                    &meta,
                    &folder.path,
                    &child_name,
                    conflict,
                    progress,
                )?;
            }
        }
        Ok(folder)
    }

    /// Decrypts a file to `dest` (through `<dest>.gvpart`, renamed only once every chunk has
    /// authenticated). An existing `dest` is replaced.
    pub fn export(
        &self,
        path: &VaultPath,
        dest: &Path,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<(), VaultError> {
        let ctx = self.ctx()?;
        let entry = self.file_entry(&ctx, path)?;
        let blob = entry
            .blob
            .ok_or(VaultError::Internal("file entry without a blob"))?;
        let part = part_path(dest);
        let result = (|| {
            let mut out = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&part)
                .map_err(VaultError::io("creating the exported file"))?;
            self.read_blob(&ctx, &blob, entry.size, &mut out, progress)?;
            out.set_modified(SystemTime::UNIX_EPOCH + Duration::from_millis(entry.modified_ms))
                .map_err(VaultError::io("writing the exported file"))?;
            out.sync_all()
                .map_err(VaultError::io("writing the exported file"))?;
            drop(out);
            self.check(&ctx)?;
            fs::rename(&part, dest).map_err(VaultError::io("renaming the exported file"))
        })();
        if result.is_err() {
            wipe_quietly(&part);
        }
        result
    }

    /// Decrypts a file into its own folder under the session root and returns its path, for
    /// opening with another program. Edits are saved by `sync_session` and on lock. Opening the
    /// same entry again returns the same path.
    pub fn open_in_session(
        &self,
        path: &VaultPath,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<PathBuf, VaultError> {
        let ctx = self.ctx()?;
        let entry = self.file_entry(&ctx, path)?;
        let folder = {
            let mut inner = self.inner()?;
            let unlocked = self.current(&mut inner, &ctx)?;
            if let Some(open) = unlocked.session.find_entry(path)
                && open.file.is_file()
            {
                return Ok(open.file.clone());
            }
            unlocked.session.allocate_folder(&self.config.session_root)
        };
        let name = session::session_file_name(&folder, path.file_name().unwrap_or("file"))?;
        let file = folder.join(name);
        match self.decrypt_into_session(&ctx, &entry, &folder, &file, progress) {
            Ok((hash, baseline)) => self.register_session_file(&ctx, path, file, hash, baseline),
            Err(e) => {
                wipe_quietly(&folder);
                Err(e)
            }
        }
    }

    fn decrypt_into_session(
        &self,
        ctx: &Ctx,
        entry: &Entry,
        folder: &Path,
        file: &Path,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<([u8; 32], Baseline), VaultError> {
        let blob = entry
            .blob
            .ok_or(VaultError::Internal("file entry without a blob"))?;
        fs::create_dir_all(folder).map_err(VaultError::io("creating a session folder"))?;
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(file)
            .map_err(VaultError::io("creating a session file"))?;
        let summary = self.read_blob(ctx, &blob, entry.size, &mut out, progress)?;
        // A fixed mtime makes any later edit visible, even on 2-second FAT clocks.
        out.set_modified(UNIX_EPOCH + Duration::from_millis(entry.modified_ms))
            .map_err(VaultError::io("writing a session file"))?;
        let meta = out
            .metadata()
            .map_err(VaultError::io("writing a session file"))?;
        Ok((summary.hash, session::stat(&meta)))
    }

    fn register_session_file(
        &self,
        ctx: &Ctx,
        path: &VaultPath,
        file: PathBuf,
        hash: [u8; 32],
        baseline: Baseline,
    ) -> Result<PathBuf, VaultError> {
        let mut inner = self.inner()?;
        let unlocked = match self.current(&mut inner, ctx) {
            Ok(unlocked) => unlocked,
            Err(e) => {
                drop(inner);
                wipe_quietly(file.parent().unwrap_or(&file));
                return Err(e);
            }
        };
        if let Some(open) = unlocked.session.find_entry(path)
            && open.file.is_file()
        {
            // Another thread opened the same entry meanwhile: keep that copy.
            let existing = open.file.clone();
            drop(inner);
            wipe_quietly(file.parent().unwrap_or(&file));
            return Ok(existing);
        }
        unlocked.session.files.retain(|f| f.entry != *path);
        let id = unlocked.session.next_id;
        unlocked.session.next_id += 1;
        unlocked.session.files.push(SessionFile {
            id,
            entry: path.clone(),
            file: file.clone(),
            hash,
            baseline,
        });
        Ok(file)
    }

    /// Decrypts every file to nowhere, checking each blob, and counts orphaned blobs.
    pub fn verify(&self, progress: Option<ProgressFn<'_>>) -> Result<VerifyReport, VaultError> {
        let ctx = self.ctx()?;
        let files: Vec<(VaultPath, [u8; 16], u64)> = {
            let mut inner = self.inner()?;
            let unlocked = self.current(&mut inner, &ctx)?;
            unlocked
                .index
                .entries
                .iter()
                .filter_map(|(p, e)| e.blob.map(|b| (p.clone(), b, e.size)))
                .collect()
        };
        let referenced: HashSet<[u8; 16]> = files.iter().map(|(_, b, _)| *b).collect();
        let orphan_blobs = store::list_blobs(&self.layout)
            .iter()
            .filter(|b| !referenced.contains(*b))
            .count();
        let total: u64 = files.iter().map(|(_, _, size)| *size).sum();
        let mut done = 0u64;
        let mut report = VerifyReport {
            files_checked: 0,
            problems: Vec::new(),
            orphan_blobs,
        };
        for (path, blob, size) in files {
            let base = done;
            let forward = |d: u64, _: u64| {
                if let Some(report) = progress {
                    report(base + d, total);
                }
            };
            let problem =
                match self.read_blob(&ctx, &blob, size, &mut std::io::sink(), Some(&forward)) {
                    Ok(_) => None,
                    Err(VaultError::BlobMissing) => Some(Problem::BlobMissing),
                    Err(VaultError::Tampered(t)) if t == tamper::TRUNCATED => {
                        Some(Problem::Truncated)
                    }
                    Err(VaultError::Tampered(t)) if t == tamper::SIZE => {
                        Some(Problem::SizeMismatch)
                    }
                    Err(VaultError::Tampered(_) | VaultError::UnsupportedVersion(_)) => {
                        Some(Problem::Tampered)
                    }
                    Err(other) => return Err(other),
                };
            if let Some(problem) = problem {
                report.problems.push((path, problem));
            }
            report.files_checked += 1;
            done += size;
        }
        Ok(report)
    }

    /// Encrypts every plain item found directly inside the vault folder into the vault root
    /// (`KeepBoth` on name clashes), then wipes the plaintext original. Items that cannot be
    /// imported (for example a name Windows cannot store) are left where they are.
    pub fn import_foreign(
        &self,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<Vec<EntryInfo>, VaultError> {
        let ctx = self.ctx()?;
        let root = VaultPath::root();
        let mut imported = Vec::new();
        for item in store::foreign_items(&self.layout) {
            let Ok(meta) = fs::symlink_metadata(&item) else {
                continue;
            };
            let Ok(name) = source_name(&item) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let result = if meta.is_dir() {
                self.import_dir(&ctx, &item, &root, &name, Conflict::KeepBoth, progress)
            } else {
                self.import_file(
                    &ctx,
                    &item,
                    &meta,
                    &root,
                    &name,
                    Conflict::KeepBoth,
                    progress,
                )
            };
            match result {
                Ok(entry) => {
                    wipe_quietly(&item);
                    imported.push(entry);
                }
                Err(e @ (VaultError::Cancelled | VaultError::Locked | VaultError::Internal(_))) => {
                    return Err(e);
                }
                Err(_) => {}
            }
        }
        let remaining = store::foreign_items(&self.layout).len();
        if let Ok(mut inner) = self.inner() {
            inner.foreign = remaining;
        }
        Ok(imported)
    }

    /// Best-effort foreign-item ingestion after create and unlock (spec A3, decision D4).
    pub(super) fn ingest_foreign(&self) {
        if !store::foreign_items(&self.layout).is_empty() {
            let _ = self.import_foreign(None);
        }
    }
}

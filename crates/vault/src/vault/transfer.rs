//! Streaming transfers: import, export, open in the session folder, verify, foreign items.
//!
//! Each transfer takes a [`Ctx`] under the state lock, streams without it (checking the epoch
//! every chunk), and re-takes the lock only to commit. Plaintext is never handed out before the
//! last chunk has authenticated: partial outputs are wiped.
//!
//! A folder import lists and name-checks the whole tree first, encrypts every file, then adds
//! everything in one index commit: it lands completely or not at all. Foreign items (plain
//! files dropped into the vault folder) are wiped only file by file, and only when the file on
//! disk is still exactly what was encrypted and its blob decrypts back to it.

// cspell:ignore gvpart

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::entries::{check_capacity, find_sibling, info, keep_both_name, require_dir};
use super::{Ctx, Vault, is_within, now_ms, resolve};
use crate::crypto::{self, StreamSummary};
use crate::error::{VaultError, tamper};
use crate::format::blob::{BlobHeader, DEFAULT_CHUNK_SIZE, encrypted_len};
use crate::format::index::{Entry, Index, MAX_ENTRIES};
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

/// `<dest>.gvpart`, or `<dest>.<suffix>.gvpart` when that name is taken.
fn part_path(dest: &Path, suffix: Option<&str>) -> PathBuf {
    let mut name: OsString = dest.as_os_str().to_owned();
    if let Some(suffix) = suffix {
        name.push(".");
        name.push(suffix);
    }
    name.push(".gvpart");
    PathBuf::from(name)
}

/// Creates a fresh export staging file next to `dest` (`create_new`: an existing file or link
/// at the name is never opened, truncated or moved).
fn create_part(dest: &Path) -> Result<(PathBuf, File), VaultError> {
    let mut part = part_path(dest, None);
    for _ in 0..8 {
        match OpenOptions::new().write(true).create_new(true).open(&part) {
            Ok(file) => return Ok((part, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let suffix: [u8; 4] = crypto::random()?;
                part = part_path(dest, Some(&session::hex(&suffix)));
            }
            Err(e) => {
                return Err(VaultError::Io {
                    context: "creating the exported file",
                    source: e,
                });
            }
        }
    }
    Err(VaultError::Internal("could not pick a free staging name"))
}

fn wipe_quietly(path: &Path) {
    let mut report = WipeReport::default();
    session::wipe_path(path, &mut report);
}

/// Why an import did not commit, which decides what happens to its new blobs.
pub(super) enum NotCommitted {
    /// Refused before the index was written: the new blobs are unreferenced and are deleted.
    Refused(VaultError),
    /// The index write failed and may still have reached the disk: the new blobs stay, and the
    /// next unlock's garbage collection removes them if the commit did not land.
    Unclear(VaultError),
}

/// How and where an import lands.
#[derive(Clone, Copy)]
struct Target<'a> {
    dest_dir: &'a VaultPath,
    name: &'a str,
    conflict: Conflict,
    progress: Option<ProgressFn<'a>>,
    /// Decrypt every new blob back before committing (foreign items, whose originals are wiped).
    verify: bool,
}

/// One file as listed before it was encrypted, and what its encryption produced.
struct ImportedFile {
    source: PathBuf,
    listed: Baseline,
    summary: StreamSummary,
}

/// What an import committed: its entry, the files it encrypted and the source folders.
struct Imported {
    entry: EntryInfo,
    files: Vec<ImportedFile>,
    folders: Vec<PathBuf>,
}

/// A file found while listing a folder to import.
struct PlannedFile {
    source: PathBuf,
    /// Components below the top folder; the last one is the file name.
    rel: Vec<String>,
    listed: Baseline,
    modified_ms: u64,
}

/// A folder tree to import, listed and name-checked before anything is encrypted.
struct TreePlan {
    /// Folders below the top one, as components below it, parents before children.
    dirs: Vec<Vec<String>>,
    files: Vec<PlannedFile>,
    /// Every source folder including the top one.
    folders: Vec<PathBuf>,
}

/// Lists `source` as it would land at `top`, checking every name, the depth and the length.
/// Links and anything that is neither a file nor a folder are skipped: never followed, never
/// imported, never wiped.
fn plan_tree(source: &Path, top: &VaultPath) -> Result<TreePlan, VaultError> {
    let mut plan = TreePlan {
        dirs: Vec::new(),
        files: Vec::new(),
        folders: vec![source.to_path_buf()],
    };
    let mut stack = vec![(source.to_path_buf(), Vec::<String>::new(), top.clone())];
    while let Some((dir, rel, at)) = stack.pop() {
        let mut children: Vec<PathBuf> = fs::read_dir(&dir)
            .map_err(VaultError::io("reading the folder to import"))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        children.sort();
        for child in children {
            let meta = fs::symlink_metadata(&child)
                .map_err(VaultError::io("reading the folder to import"))?;
            if meta.file_type().is_symlink() || !(meta.is_dir() || meta.is_file()) {
                continue;
            }
            let name = source_name(&child)?;
            let child_at = at.join(&name)?; // depth and length are checked here
            let mut child_rel = rel.clone();
            child_rel.push(name);
            if meta.is_dir() {
                plan.dirs.push(child_rel.clone());
                plan.folders.push(child.clone());
                stack.push((child, child_rel, child_at));
            } else {
                plan.files.push(PlannedFile {
                    source: child,
                    rel: child_rel,
                    listed: session::stat(&meta),
                    modified_ms: meta.modified().map_or_else(|_| now_ms(), ms_of),
                });
            }
        }
        if plan.dirs.len() + plan.files.len() >= MAX_ENTRIES {
            return Err(VaultError::LimitExceeded(
                "a vault holds at most 8192 files and folders",
            ));
        }
    }
    Ok(plan)
}

/// Where one new entry lands in `next`.
struct Staged {
    path: VaultPath,
    /// The blob of a file this one replaced.
    superseded: Option<[u8; 16]>,
}

/// Adds `entry` named `name` to `dir` in `next`, resolving a name clash by `conflict`. With
/// `merge_dirs` a folder meeting an existing folder merges into it.
fn stage_entry(
    next: &mut Index,
    dir: &VaultPath,
    name: &str,
    conflict: Conflict,
    entry: Entry,
    merge_dirs: bool,
) -> Result<Staged, VaultError> {
    let target = dir.join(name)?;
    let existing = find_sibling(next, dir, name)
        .and_then(|p| next.entries.get(&p).map(|e| (p, e.kind, e.blob)));
    let (path, superseded) = match (existing, conflict) {
        (None, _) => (target, None),
        (Some((path, EntryKind::Dir, _)), _) if merge_dirs && entry.kind == EntryKind::Dir => {
            return Ok(Staged {
                path,
                superseded: None,
            });
        }
        (Some((path, EntryKind::File, old)), Conflict::Replace)
            if entry.kind == EntryKind::File =>
        {
            (path, old)
        }
        (Some(_), Conflict::KeepBoth) => (keep_both_name(next, dir, name)?, None),
        (Some(_), _) => return Err(VaultError::Exists(target.as_str().to_owned())),
    };
    next.entries.insert(path.clone(), entry);
    Ok(Staged { path, superseded })
}

/// `Some(meta)` when the original at `file.source` is still exactly what was encrypted: the
/// same plain file, length and time as listed, and the same content hash.
fn unchanged_original(file: &ImportedFile) -> Option<fs::Metadata> {
    let meta = fs::symlink_metadata(&file.source).ok()?;
    if !meta.is_file()
        || session::stat(&meta) != file.listed
        || meta.len() != file.summary.plain_len
    {
        return None;
    }
    let hash = File::open(&file.source)
        .and_then(|mut f| crypto::hash_reader(&mut f))
        .ok()?;
    (hash == file.summary.hash).then_some(meta)
}

/// Wipes the originals of a committed foreign import: only the files that are unchanged, then
/// the folders that became empty (deepest first, never recursively).
fn wipe_originals(imported: &Imported) {
    for file in &imported.files {
        if let Some(meta) = unchanged_original(file) {
            let _ = session::wipe_file(&file.source, &meta);
        }
    }
    let mut folders = imported.folders.clone();
    folders.sort_by_key(|f| std::cmp::Reverse(f.components().count()));
    for folder in folders {
        let _ = fs::remove_dir(folder);
    }
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
    /// The source is left in place. A folder lands in one index commit, completely or not at all.
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
        let target = Target {
            dest_dir,
            name: &name,
            conflict: on_conflict,
            progress,
            verify: false,
        };
        let imported = if meta.is_dir() {
            self.import_dir(&ctx, source, target)?
        } else {
            self.import_file(&ctx, source, &meta, target)?
        };
        Ok(imported.entry)
    }

    /// Deletes the new blobs of a failed import unless its commit may have landed.
    fn discard(&self, failure: NotCommitted, blobs: &[[u8; 16]]) -> VaultError {
        match failure {
            NotCommitted::Refused(e) => {
                self.remove_blobs(blobs);
                e
            }
            NotCommitted::Unclear(e) => e,
        }
    }

    /// Decrypts a just-written blob to nowhere and checks it yields the same plaintext.
    fn verify_blob(
        &self,
        ctx: &Ctx,
        blob: &[u8; 16],
        summary: &StreamSummary,
    ) -> Result<(), VaultError> {
        let check = self.read_blob(ctx, blob, summary.plain_len, &mut std::io::sink(), None)?;
        if check.hash == summary.hash {
            Ok(())
        } else {
            Err(VaultError::Tampered(tamper::BLOB))
        }
    }

    fn import_file(
        &self,
        ctx: &Ctx,
        source: &Path,
        meta: &fs::Metadata,
        target: Target<'_>,
    ) -> Result<Imported, VaultError> {
        {
            // Cheap early refusals; everything is checked again at commit.
            let mut inner = self.inner()?;
            let unlocked = self.current(&mut inner, ctx)?;
            require_dir(&unlocked.index, target.dest_dir)?;
            if target.conflict == Conflict::Fail
                && find_sibling(&unlocked.index, target.dest_dir, target.name).is_some()
            {
                return Err(VaultError::Exists(
                    target.dest_dir.join(target.name)?.as_str().to_owned(),
                ));
            }
        }
        if let Some(max) = self.config.max_blob_bytes
            && encrypted_len(meta.len(), DEFAULT_CHUNK_SIZE) > max
        {
            return Err(VaultError::FileTooLarge);
        }
        let listed = session::stat(meta);
        let modified_ms = meta.modified().map_or_else(|_| now_ms(), ms_of);
        let mut file = File::open(source).map_err(VaultError::io("opening the file to import"))?;
        let (blob, summary) = self.write_blob(ctx, &mut file, meta.len(), target.progress)?;
        drop(file);
        if target.verify
            && let Err(e) = self.verify_blob(ctx, &blob, &summary)
        {
            self.remove_blobs(&[blob]);
            return Err(e);
        }
        let entry = Entry::file(blob, summary.plain_len, modified_ms);
        match self.commit_new_entry(ctx, target, entry) {
            Ok(entry) => Ok(Imported {
                entry,
                files: vec![ImportedFile {
                    source: source.to_path_buf(),
                    listed,
                    summary,
                }],
                folders: Vec::new(),
            }),
            Err(failure) => Err(self.discard(failure, &[blob])),
        }
    }

    /// Adds one file entry, resolving a name conflict, in one commit.
    fn commit_new_entry(
        &self,
        ctx: &Ctx,
        target: Target<'_>,
        entry: Entry,
    ) -> Result<EntryInfo, NotCommitted> {
        let mut inner = self.inner().map_err(NotCommitted::Refused)?;
        let unlocked = self
            .current(&mut inner, ctx)
            .map_err(NotCommitted::Refused)?;
        require_dir(&unlocked.index, target.dest_dir).map_err(NotCommitted::Refused)?;
        let mut next = unlocked.index.clone();
        let staged = stage_entry(
            &mut next,
            target.dest_dir,
            target.name,
            target.conflict,
            entry,
            false,
        )
        .map_err(NotCommitted::Refused)?;
        check_capacity(&next, 0).map_err(NotCommitted::Refused)?;
        let listed = next
            .entries
            .get(&staged.path)
            .map(|e| info(&staged.path, e))
            .ok_or(NotCommitted::Refused(VaultError::Internal(
                "entry vanished",
            )))?;
        self.commit(unlocked, next).map_err(NotCommitted::Unclear)?;
        drop(inner);
        if let Some(old) = staged.superseded {
            self.remove_blobs(&[old]);
        }
        Ok(listed)
    }

    /// Lists, encrypts and commits a whole folder tree (see the module docs).
    fn import_dir(
        &self,
        ctx: &Ctx,
        source: &Path,
        target: Target<'_>,
    ) -> Result<Imported, VaultError> {
        let plan = {
            let mut inner = self.inner()?;
            let unlocked = self.current(&mut inner, ctx)?;
            require_dir(&unlocked.index, target.dest_dir)?;
            if target.conflict == Conflict::Fail
                && find_sibling(&unlocked.index, target.dest_dir, target.name).is_some()
            {
                return Err(VaultError::Exists(
                    target.dest_dir.join(target.name)?.as_str().to_owned(),
                ));
            }
            let top = target.dest_dir.join(target.name)?;
            drop(inner);
            let plan = plan_tree(source, &top)?;
            let mut inner = self.inner()?;
            let unlocked = self.current(&mut inner, ctx)?;
            check_capacity(&unlocked.index, 1 + plan.dirs.len() + plan.files.len())?;
            plan
        };
        let written = self.encrypt_tree(ctx, &plan, target)?;
        let blobs: Vec<[u8; 16]> = written.iter().map(|(blob, _)| *blob).collect();
        let entry = self
            .commit_tree(ctx, target, &plan, &written)
            .map_err(|failure| self.discard(failure, &blobs))?;
        let files = plan
            .files
            .into_iter()
            .zip(written)
            .map(|(file, (_, summary))| ImportedFile {
                source: file.source,
                listed: file.listed,
                summary,
            })
            .collect();
        Ok(Imported {
            entry,
            files,
            folders: plan.folders,
        })
    }

    /// Encrypts every planned file into a new blob; on any failure all of them are deleted.
    fn encrypt_tree(
        &self,
        ctx: &Ctx,
        plan: &TreePlan,
        target: Target<'_>,
    ) -> Result<Vec<([u8; 16], StreamSummary)>, VaultError> {
        if let Some(max) = self.config.max_blob_bytes
            && plan
                .files
                .iter()
                .any(|f| encrypted_len(f.listed.0, DEFAULT_CHUNK_SIZE) > max)
        {
            return Err(VaultError::FileTooLarge);
        }
        let total: u64 = plan.files.iter().map(|f| f.listed.0).sum();
        let mut done = 0u64;
        let mut written: Vec<([u8; 16], StreamSummary)> = Vec::with_capacity(plan.files.len());
        for file in &plan.files {
            let base = done;
            let forward = |d: u64, _: u64| {
                if let Some(report) = target.progress {
                    report(base.saturating_add(d), total);
                }
            };
            let result = File::open(&file.source)
                .map_err(VaultError::io("opening the file to import"))
                .and_then(|mut reader| {
                    self.write_blob(ctx, &mut reader, file.listed.0, Some(&forward))
                })
                .and_then(|(blob, summary)| {
                    if target.verify
                        && let Err(e) = self.verify_blob(ctx, &blob, &summary)
                    {
                        self.remove_blobs(&[blob]);
                        return Err(e);
                    }
                    Ok((blob, summary))
                });
            match result {
                Ok((blob, summary)) => {
                    done = done.saturating_add(summary.plain_len);
                    written.push((blob, summary));
                }
                Err(e) => {
                    let blobs: Vec<[u8; 16]> = written.iter().map(|(b, _)| *b).collect();
                    self.remove_blobs(&blobs);
                    return Err(e);
                }
            }
        }
        Ok(written)
    }

    /// Adds the top folder, every folder and every file of `plan` in one commit.
    fn commit_tree(
        &self,
        ctx: &Ctx,
        target: Target<'_>,
        plan: &TreePlan,
        written: &[([u8; 16], StreamSummary)],
    ) -> Result<EntryInfo, NotCommitted> {
        let refused = NotCommitted::Refused;
        let lost = || NotCommitted::Refused(VaultError::Internal("import plan out of step"));
        let mut inner = self.inner().map_err(refused)?;
        let unlocked = self.current(&mut inner, ctx).map_err(refused)?;
        require_dir(&unlocked.index, target.dest_dir).map_err(refused)?;
        let mut next = unlocked.index.clone();
        let now = now_ms();
        let top = stage_entry(
            &mut next,
            target.dest_dir,
            target.name,
            target.conflict,
            Entry::dir(now),
            target.conflict == Conflict::Replace,
        )
        .map_err(refused)?;
        let mut placed: HashMap<&[String], VaultPath> = HashMap::new();
        placed.insert(&[], top.path.clone());
        for rel in &plan.dirs {
            let (name, parent) = rel.split_last().ok_or_else(lost)?;
            let parent = placed.get(parent).ok_or_else(lost)?.clone();
            let staged = stage_entry(
                &mut next,
                &parent,
                name,
                target.conflict,
                Entry::dir(now),
                true,
            )
            .map_err(refused)?;
            placed.insert(rel, staged.path);
        }
        let mut superseded = Vec::new();
        for (file, (blob, summary)) in plan.files.iter().zip(written) {
            let (name, parent) = file.rel.split_last().ok_or_else(lost)?;
            let parent = placed.get(parent).ok_or_else(lost)?.clone();
            let entry = Entry::file(*blob, summary.plain_len, file.modified_ms);
            let staged = stage_entry(&mut next, &parent, name, target.conflict, entry, false)
                .map_err(refused)?;
            superseded.extend(staged.superseded);
        }
        check_capacity(&next, 0).map_err(refused)?;
        let listed = next
            .entries
            .get(&top.path)
            .map(|e| info(&top.path, e))
            .ok_or_else(lost)?;
        self.commit(unlocked, next).map_err(NotCommitted::Unclear)?;
        drop(inner);
        self.remove_blobs(&superseded);
        Ok(listed)
    }

    /// Refuses an export destination inside the vault folder or the session folder.
    fn check_export_dest(&self, dest: &Path) -> Result<(), VaultError> {
        let resolved = resolve(dest)?;
        if is_within(&resolved, &self.resolved_root)
            || is_within(&resolved, &self.resolved_session_root)
        {
            return Err(VaultError::InvalidPath(
                "an export cannot go into the vault or session folder",
            ));
        }
        Ok(())
    }

    /// Decrypts a file to `dest` (through a fresh `<dest>.gvpart`, renamed only once every
    /// chunk has authenticated). An existing `dest` is replaced. A destination inside the vault
    /// or session folder is refused.
    pub fn export(
        &self,
        path: &VaultPath,
        dest: &Path,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<(), VaultError> {
        let ctx = self.ctx()?;
        self.check_export_dest(dest)?;
        let entry = self.file_entry(&ctx, path)?;
        let blob = entry
            .blob
            .ok_or(VaultError::Internal("file entry without a blob"))?;
        let (part, mut out) = create_part(dest)?;
        let result = (|| {
            self.read_blob(&ctx, &blob, entry.size, &mut out, progress)?;
            out.set_modified(SystemTime::UNIX_EPOCH + Duration::from_millis(entry.modified_ms))
                .map_err(VaultError::io("writing the exported file"))?;
            out.sync_all()
                .map_err(VaultError::io("writing the exported file"))?;
            self.check(&ctx)?;
            fs::rename(&part, dest).map_err(VaultError::io("renaming the exported file"))
        })();
        drop(out);
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
    /// (`KeepBoth` on name clashes). A folder lands in one commit or not at all. Afterwards only
    /// originals that are unchanged since they were listed, and whose blobs decrypt back to
    /// them, are wiped; links, new files and changed files stay and are still foreign.
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
            if meta.file_type().is_symlink() || !(meta.is_dir() || meta.is_file()) {
                continue;
            }
            let target = Target {
                dest_dir: &root,
                name: &name,
                conflict: Conflict::KeepBoth,
                progress,
                verify: true,
            };
            let result = if meta.is_dir() {
                self.import_dir(&ctx, &item, target)
            } else {
                self.import_file(&ctx, &item, &meta, target)
            };
            match result {
                Ok(done) => {
                    wipe_originals(&done);
                    imported.push(done.entry);
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

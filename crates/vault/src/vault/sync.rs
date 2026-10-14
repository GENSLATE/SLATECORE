//! Re-encrypting edits made in the session folder (research/vault.md 6.5).
//!
//! Polling, idempotent and cheap when nothing changed: a stat per open file, a hash only when
//! `(length, mtime)` moved, a new blob only when the hash moved. All changes of one pass are
//! committed in one index write. New files saved next to an opened file ("Save As") are imported
//! as its siblings on lock only.

use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::entries::{check_capacity, find_sibling, keep_both_name, require_dir};
use super::{Ctx, Unlocked, Vault, gate, not_unlocked, now_ms};
use crate::EntryKind;
use crate::SyncReport;
use crate::crypto;
use crate::error::VaultError;
use crate::format::index::{Entry, Index};
use crate::path::{VaultPath, validate_component};
use crate::session::{self, Baseline, SessionFile};

/// Files modified more recently than this are left for the next pass (editors save in bursts).
const SETTLE: Duration = Duration::from_millis(500);

/// Per-file problems: the entry (or its folder) and a message.
type Failures = Vec<(VaultPath, String)>;

enum Outcome {
    Unchanged,
    Forget,
    Touched(Baseline),
    Updated {
        blob: [u8; 16],
        size: u64,
        modified_ms: u64,
        hash: [u8; 32],
        baseline: Baseline,
    },
    Failed(String),
}

/// A new file found next to an opened one, already encrypted into `blob`.
struct Sibling {
    owner: u64,
    name: String,
    file: PathBuf,
    blob: [u8; 16],
    size: u64,
    modified_ms: u64,
    hash: [u8; 32],
    baseline: Baseline,
}

fn recently_modified(baseline: &Baseline) -> bool {
    baseline
        .1
        .is_some_and(|t| t.elapsed().is_ok_and(|age| age < SETTLE))
}

fn modified_ms(baseline: &Baseline) -> u64 {
    baseline
        .1
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or_else(now_ms, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl Vault {
    /// Saves edits made to opened session files back into the vault.
    pub fn sync_session(&self) -> Result<SyncReport, VaultError> {
        let _sync_gate = gate(&self.sync_gate);
        self.sync_pass(false)
    }

    /// One pass. `force` (lock, exit) hashes every file regardless of its stat, and imports new
    /// sibling files. Only `Cancelled` and commit failures are errors; per-file problems are in
    /// `failed`.
    pub(super) fn sync_pass(&self, force: bool) -> Result<SyncReport, VaultError> {
        let (ctx, files) = {
            let inner = self.inner()?;
            let unlocked = inner
                .unlocked
                .as_ref()
                .ok_or_else(|| not_unlocked(&inner))?;
            let ctx = Ctx {
                key: unlocked.key.clone(),
                vault_id: unlocked.vault_id,
                epoch: self.epoch.load(std::sync::atomic::Ordering::SeqCst),
            };
            (ctx, unlocked.session.files.clone())
        };
        let mut outcomes = Vec::with_capacity(files.len());
        let mut new_blobs = Vec::new();
        for file in &files {
            match self.sync_one(&ctx, file, force) {
                Ok(outcome) => {
                    if let Outcome::Updated { blob, .. } = &outcome {
                        new_blobs.push(*blob);
                    }
                    outcomes.push((file.id, outcome));
                }
                Err(e) => {
                    self.remove_blobs(&new_blobs);
                    return Err(e);
                }
            }
        }
        let (siblings, sibling_failures) = if force {
            match self.collect_siblings(&ctx, &files) {
                Ok(found) => found,
                Err(e) => {
                    self.remove_blobs(&new_blobs);
                    return Err(e);
                }
            }
        } else {
            (Vec::new(), Vec::new())
        };
        let mut report = SyncReport {
            failed: sibling_failures,
            ..SyncReport::default()
        };
        self.apply(&ctx, outcomes, siblings, &mut report)?;
        Ok(report)
    }

    /// Deletes blobs no index references (best effort).
    pub(super) fn remove_blobs(&self, blobs: &[[u8; 16]]) {
        for blob in blobs {
            let _ = fs::remove_file(self.layout.blob(blob));
        }
    }

    /// Checks one session file; `Err` only for cancellation.
    fn sync_one(&self, ctx: &Ctx, file: &SessionFile, force: bool) -> Result<Outcome, VaultError> {
        let meta = match fs::metadata(&file.file) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Outcome::Forget),
            Err(e) => return Ok(Outcome::Failed(e.to_string())),
        };
        let baseline = session::stat(&meta);
        if !force && (baseline == file.baseline || recently_modified(&baseline)) {
            return Ok(Outcome::Unchanged);
        }
        let hash = match File::open(&file.file).and_then(|mut f| crypto::hash_reader(&mut f)) {
            Ok(hash) => hash,
            Err(e) => return Ok(Outcome::Failed(e.to_string())),
        };
        if hash == file.hash {
            return Ok(Outcome::Touched(baseline));
        }
        let mut reader = match File::open(&file.file) {
            Ok(reader) => reader,
            Err(e) => return Ok(Outcome::Failed(e.to_string())),
        };
        match self.write_blob(ctx, &mut reader, meta.len(), None) {
            Ok((blob, summary)) => Ok(Outcome::Updated {
                blob,
                size: summary.plain_len,
                modified_ms: modified_ms(&baseline),
                hash: summary.hash,
                baseline,
            }),
            Err(VaultError::Cancelled) => Err(VaultError::Cancelled),
            Err(e) => Ok(Outcome::Failed(e.to_string())),
        }
    }

    /// Encrypts new regular files found next to opened files (lock only).
    fn collect_siblings(
        &self,
        ctx: &Ctx,
        files: &[SessionFile],
    ) -> Result<(Vec<Sibling>, Failures), VaultError> {
        let registered: HashSet<&Path> = files.iter().map(|f| f.file.as_path()).collect();
        let mut folders = HashSet::new();
        let mut found = Vec::new();
        let mut failed = Vec::new();
        for owner in files {
            let Some(folder) = owner.folder() else {
                continue;
            };
            if !folders.insert(folder.to_path_buf()) {
                continue;
            }
            let Ok(entries) = fs::read_dir(folder) else {
                continue;
            };
            for item in entries.flatten() {
                let path = item.path();
                let is_file = item.file_type().is_ok_and(|t| t.is_file());
                let Some(name) = item.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if !is_file
                    || registered.contains(path.as_path())
                    || session::is_ignored_sibling(&name)
                {
                    continue;
                }
                let dir = owner.entry.parent().unwrap_or_else(VaultPath::root);
                if validate_component(&name).is_err() {
                    failed.push((
                        dir,
                        "a new file's name cannot be stored in the vault".to_owned(),
                    ));
                    continue;
                }
                match self.encrypt_sibling(ctx, owner.id, name, path) {
                    Ok(sibling) => found.push(sibling),
                    Err(VaultError::Cancelled) => {
                        let blobs: Vec<[u8; 16]> = found.iter().map(|s| s.blob).collect();
                        self.remove_blobs(&blobs);
                        return Err(VaultError::Cancelled);
                    }
                    Err(e) => failed.push((dir, e.to_string())),
                }
            }
        }
        Ok((found, failed))
    }

    fn encrypt_sibling(
        &self,
        ctx: &Ctx,
        owner: u64,
        name: String,
        path: PathBuf,
    ) -> Result<Sibling, VaultError> {
        let meta = fs::metadata(&path).map_err(VaultError::io("reading a new session file"))?;
        let baseline = session::stat(&meta);
        let mut reader = File::open(&path).map_err(VaultError::io("reading a new session file"))?;
        let (blob, summary) = self.write_blob(ctx, &mut reader, meta.len(), None)?;
        Ok(Sibling {
            owner,
            name,
            file: path,
            blob,
            size: summary.plain_len,
            modified_ms: modified_ms(&baseline),
            hash: summary.hash,
            baseline,
        })
    }

    /// Stages every change into one index commit and updates the registry.
    fn apply(
        &self,
        ctx: &Ctx,
        outcomes: Vec<(u64, Outcome)>,
        siblings: Vec<Sibling>,
        report: &mut SyncReport,
    ) -> Result<(), VaultError> {
        let all_new: Vec<[u8; 16]> = outcomes
            .iter()
            .filter_map(|(_, o)| {
                if let Outcome::Updated { blob, .. } = o {
                    Some(*blob)
                } else {
                    None
                }
            })
            .chain(siblings.iter().map(|s| s.blob))
            .collect();
        let mut inner = self.inner()?;
        let unlocked = match self.current(&mut inner, ctx) {
            Ok(unlocked) => unlocked,
            Err(e) => {
                drop(inner);
                self.remove_blobs(&all_new);
                return Err(e);
            }
        };
        let mut next = unlocked.index.clone();
        let mut staged: Vec<(u64, [u8; 32], Baseline)> = Vec::new();
        let mut unused = Vec::new();
        let mut superseded = Vec::new();
        let mut forget = Vec::new();
        for (id, outcome) in outcomes {
            stage_outcome(
                unlocked,
                &mut next,
                id,
                outcome,
                report,
                &mut Staging {
                    staged: &mut staged,
                    unused: &mut unused,
                    superseded: &mut superseded,
                    forget: &mut forget,
                },
            );
        }
        let mut new_files = Vec::new();
        for sibling in siblings {
            match stage_sibling(unlocked, &mut next, &sibling) {
                Ok(path) => {
                    report.imported_new.push(path.clone());
                    new_files.push((path, sibling));
                }
                Err((dir, message)) => {
                    unused.push(sibling.blob);
                    report.failed.push((dir, message));
                }
            }
        }
        if (!staged.is_empty() || !new_files.is_empty())
            && let Err(e) = self.commit(unlocked, next)
        {
            // The write may have reached the disk: keep the new blobs; the next unlock's
            // garbage collection removes them if the commit did not land.
            return Err(e);
        }
        update_registry(unlocked, &staged, new_files, &forget);
        drop(inner);
        self.remove_blobs(&superseded);
        self.remove_blobs(&unused);
        Ok(())
    }
}

struct Staging<'a> {
    staged: &'a mut Vec<(u64, [u8; 32], Baseline)>,
    unused: &'a mut Vec<[u8; 16]>,
    superseded: &'a mut Vec<[u8; 16]>,
    forget: &'a mut Vec<u64>,
}

fn stage_outcome(
    unlocked: &mut Unlocked,
    next: &mut Index,
    id: u64,
    outcome: Outcome,
    report: &mut SyncReport,
    staging: &mut Staging<'_>,
) {
    let Some(file) = unlocked.session.files.iter_mut().find(|f| f.id == id) else {
        // The entry was deleted while we worked.
        if let Outcome::Updated { blob, .. } = outcome {
            staging.unused.push(blob);
        }
        return;
    };
    match outcome {
        Outcome::Unchanged => {}
        Outcome::Forget => staging.forget.push(id),
        Outcome::Touched(baseline) => file.baseline = baseline,
        Outcome::Failed(message) => report.failed.push((file.entry.clone(), message)),
        Outcome::Updated {
            blob,
            size,
            modified_ms,
            hash,
            baseline,
        } => match next.entries.get_mut(&file.entry) {
            Some(entry) if entry.kind == EntryKind::File => {
                staging.superseded.extend(entry.blob);
                *entry = Entry::file(blob, size, modified_ms);
                report.updated.push(file.entry.clone());
                staging.staged.push((id, hash, baseline));
            }
            _ => staging.unused.push(blob),
        },
    }
}

/// Places a new sibling next to its owner's entry; `Err` carries the folder and a message.
fn stage_sibling(
    unlocked: &Unlocked,
    next: &mut Index,
    sibling: &Sibling,
) -> Result<VaultPath, (VaultPath, String)> {
    let owner = unlocked
        .session
        .files
        .iter()
        .find(|f| f.id == sibling.owner);
    let dir = owner
        .and_then(|o| o.entry.parent())
        .unwrap_or_else(VaultPath::root);
    let fail = |e: VaultError| (dir.clone(), e.to_string());
    require_dir(next, &dir).map_err(fail)?;
    check_capacity(next, 1).map_err(fail)?;
    let path = match find_sibling(next, &dir, &sibling.name) {
        None => dir.join(&sibling.name),
        Some(_) => keep_both_name(next, &dir, &sibling.name),
    }
    .map_err(fail)?;
    next.entries.insert(
        path.clone(),
        Entry::file(sibling.blob, sibling.size, sibling.modified_ms),
    );
    Ok(path)
}

fn update_registry(
    unlocked: &mut Unlocked,
    staged: &[(u64, [u8; 32], Baseline)],
    new_files: Vec<(VaultPath, Sibling)>,
    forget: &[u64],
) {
    for (id, hash, baseline) in staged {
        if let Some(file) = unlocked.session.files.iter_mut().find(|f| f.id == *id) {
            file.hash = *hash;
            file.baseline = *baseline;
        }
    }
    for (path, sibling) in new_files {
        let id = unlocked.session.next_id;
        unlocked.session.next_id += 1;
        unlocked.session.files.push(SessionFile {
            id,
            entry: path,
            file: sibling.file,
            hash: sibling.hash,
            baseline: sibling.baseline,
        });
    }
    unlocked.session.files.retain(|f| !forget.contains(&f.id));
}

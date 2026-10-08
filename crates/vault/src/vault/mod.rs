//! The [`Vault`] state machine (research/vault.md 4.8, 8.2).
//!
//! Locking order, always: `unlock_gate` (create, unlock, `change_password`, lock), then
//! `sync_gate` (session syncs, lock), then `inner` (all state, held only for short sections).
//! Long transfers copy what they need under `inner`, release it, stream, and re-take it to
//! commit. `lock` and `unlock` bump the `epoch`; every streaming loop re-checks it and aborts with
//! [`VaultError::Cancelled`], removing its partial output, so a lock never waits for an import.
//! A poisoned `inner` fails closed: the key is dropped, the session wiped, calls return
//! `Internal`.

mod entries;
mod sync;
mod transfer;

use std::fmt;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use secrecy::SecretString;
use zeroize::Zeroizing;

use crate::crypto::{self, SecretKey};
use crate::error::VaultError;
use crate::format::header::HeaderSlot;
use crate::format::index::Index;
use crate::kdf::{self, KdfParams};
use crate::session::{self, Session};
use crate::store::{self, Layout};
use crate::throttle::Throttle;
use crate::{LockPolicy, LockReport, StartupReport, VaultConfig, VaultState, VaultStatus};

/// `kdf_limit` value meaning "the launcher set none" (the default cap applies).
const NO_LIMIT: u64 = u64::MAX;

/// Thread-safe; share as `Arc<Vault>`.
pub struct Vault {
    config: VaultConfig,
    layout: Layout,
    /// `config.root` and `config.session_root` with links resolved, for containment checks.
    resolved_root: PathBuf,
    resolved_session_root: PathBuf,
    /// Holds the OS lock on `vault.lock` for the life of the handle.
    _instance_lock: File,
    epoch: AtomicU64,
    kdf_limit: AtomicU64,
    unlock_gate: Mutex<()>,
    sync_gate: Mutex<()>,
    inner: Mutex<Inner>,
}

struct Inner {
    /// A header slot file exists (the vault has been created).
    exists: bool,
    /// The winning header as last read (for `status`).
    header: Option<HeaderSlot>,
    throttle: Throttle,
    foreign: usize,
    /// Session files a lock could not delete.
    leftovers: usize,
    unlocked: Option<Unlocked>,
    poisoned: bool,
}

struct Unlocked {
    key: Arc<SecretKey>,
    vault_id: [u8; 16],
    header: HeaderSlot,
    generation: u64,
    /// The index slot holding `generation`; commits go to the other one.
    index_slot: usize,
    index: Index,
    session: Session,
}

/// What a transfer needs while it runs without the state lock.
#[derive(Clone)]
struct Ctx {
    key: Arc<SecretKey>,
    vault_id: [u8; 16],
    epoch: u64,
}

fn gate(mutex: &Mutex<()>) -> MutexGuard<'_, ()> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn not_unlocked(inner: &Inner) -> VaultError {
    if inner.exists {
        VaultError::Locked
    } else {
        VaultError::Uninitialized
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// `path` made absolute with links resolved as far as it exists; the components that do not
/// exist yet are appended as written.
fn resolve(path: &Path) -> Result<PathBuf, VaultError> {
    let absolute = std::path::absolute(path).map_err(VaultError::io("resolving a vault folder"))?;
    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        if let Ok(mut resolved) = fs::canonicalize(existing) {
            resolved.extend(missing.iter().rev());
            return Ok(resolved);
        }
        let (Some(parent), Some(name)) = (existing.parent(), existing.file_name()) else {
            return Err(VaultError::InvalidConfig(
                "a vault folder path cannot be resolved",
            ));
        };
        missing.push(name.to_owned());
        existing = parent;
    }
}

/// `true` when the resolved `path` is `root` or lies inside it. Names compare case-insensitively
/// on Windows, as its file systems do.
pub(crate) fn is_within(path: &Path, root: &Path) -> bool {
    let key = |p: &Path| -> Vec<String> {
        p.components()
            .map(|c| {
                let name = c.as_os_str().to_string_lossy();
                if cfg!(windows) {
                    name.to_lowercase()
                } else {
                    name.into_owned()
                }
            })
            .collect()
    };
    let (path, root) = (key(path), key(root));
    path.len() >= root.len() && path[..root.len()] == root[..]
}

/// Wraps `key` under a new KEK into a header slot.
fn seal_header(
    kek: &SecretKey,
    key: &SecretKey,
    generation: u64,
    vault_id: [u8; 16],
    kdf: KdfParams,
    salt: [u8; 16],
) -> Result<HeaderSlot, VaultError> {
    let mut slot = HeaderSlot {
        generation,
        vault_id,
        kdf,
        salt,
        wrap_nonce: crypto::random()?,
        wrapped_vk: [0u8; 48],
    };
    slot.wrapped_vk = crypto::wrap_vault_key(kek, &slot.wrap_nonce, key, &slot.wrap_aad())?;
    Ok(slot)
}

impl Vault {
    /// Takes `vault.lock` (`Busy` if another instance holds it), wipes stale session folders,
    /// scans for foreign items. Never runs the KDF.
    ///
    /// The two folders must not overlap ([`VaultError::InvalidConfig`]); this is checked before
    /// either is created.
    pub fn open(config: VaultConfig) -> Result<(Vault, StartupReport), VaultError> {
        let (root, session_root) = (resolve(&config.root)?, resolve(&config.session_root)?);
        if is_within(&root, &session_root) || is_within(&session_root, &root) {
            return Err(VaultError::InvalidConfig(
                "the session folder overlaps the vault folder",
            ));
        }
        fs::create_dir_all(&config.root).map_err(VaultError::io("creating the vault folder"))?;
        fs::create_dir_all(&config.session_root)
            .map_err(VaultError::io("creating the session folder"))?;
        let resolved_root =
            fs::canonicalize(&config.root).map_err(VaultError::io("resolving the vault folder"))?;
        let resolved_session_root = fs::canonicalize(&config.session_root)
            .map_err(VaultError::io("resolving the session folder"))?;
        let layout = Layout::new(&config.root);
        let instance_lock = store::take_instance_lock(&layout)?;
        // We own the lock, so no other process owns the session folders: wipe them all.
        let wipe = session::wipe_sessions(&config.session_root);
        let headers = store::read_headers(&layout);
        let throttle = Throttle::load(layout.guard.clone(), Instant::now(), SystemTime::now());
        let foreign = store::foreign_items(&layout).len();
        let inner = Inner {
            exists: headers.exists,
            header: headers.winner.ok(),
            throttle,
            foreign,
            leftovers: wipe.left.len(),
            unlocked: None,
            poisoned: false,
        };
        let vault = Vault {
            config,
            layout,
            resolved_root,
            resolved_session_root,
            _instance_lock: instance_lock,
            epoch: AtomicU64::new(0),
            kdf_limit: AtomicU64::new(NO_LIMIT),
            unlock_gate: Mutex::new(()),
            sync_gate: Mutex::new(()),
            inner: Mutex::new(inner),
        };
        Ok((vault, startup_report(wipe)))
    }

    /// The state lock; fails closed after a panic poisoned it.
    fn inner(&self) -> Result<MutexGuard<'_, Inner>, VaultError> {
        let guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                self.inner.clear_poison();
                let mut guard = poisoned.into_inner();
                guard.unlocked = None;
                guard.poisoned = true;
                self.epoch.fetch_add(1, Ordering::SeqCst);
                let _ = session::wipe_sessions(&self.config.session_root);
                guard
            }
        };
        if guard.poisoned {
            return Err(VaultError::Internal(
                "the vault failed closed after an internal panic",
            ));
        }
        Ok(guard)
    }

    fn ctx(&self) -> Result<Ctx, VaultError> {
        let inner = self.inner()?;
        let unlocked = inner
            .unlocked
            .as_ref()
            .ok_or_else(|| not_unlocked(&inner))?;
        Ok(Ctx {
            key: Arc::clone(&unlocked.key),
            vault_id: unlocked.vault_id,
            epoch: self.epoch.load(Ordering::SeqCst),
        })
    }

    /// `Cancelled` once a lock or unlock happened since `ctx` was taken.
    fn check(&self, ctx: &Ctx) -> Result<(), VaultError> {
        if self.epoch.load(Ordering::SeqCst) == ctx.epoch {
            Ok(())
        } else {
            Err(VaultError::Cancelled)
        }
    }

    /// The unlocked state for a commit, provided nothing locked the vault since `ctx`.
    fn current<'a>(&self, inner: &'a mut Inner, ctx: &Ctx) -> Result<&'a mut Unlocked, VaultError> {
        self.check(ctx)?;
        inner.unlocked.as_mut().ok_or(VaultError::Cancelled)
    }

    /// Writes `index` as the next generation into the slot not holding the current one; the
    /// in-memory index changes only on success. On failure the write may or may not have reached
    /// the disk, so callers keep any new blobs it references (the next unlock collects them).
    fn commit(&self, unlocked: &mut Unlocked, index: Index) -> Result<(), VaultError> {
        let generation = unlocked.generation + 1;
        let slot = 1 - unlocked.index_slot;
        store::commit_index(
            &self.layout,
            &unlocked.key,
            &unlocked.vault_id,
            generation,
            &index,
            slot,
        )?;
        unlocked.generation = generation;
        unlocked.index_slot = slot;
        unlocked.index = index;
        Ok(())
    }

    fn kdf_limit(&self) -> u64 {
        let limit = self.kdf_limit.load(Ordering::SeqCst);
        kdf::memory_limit((limit != NO_LIMIT).then_some(limit))
    }

    /// Caps the memory the KDF may allocate. `None` restores the default cap, 1 GiB (the
    /// largest header the format accepts); larger values are clamped to it. The launcher can
    /// lower it from the PC's available memory so an oversized or hostile header fails with
    /// `OutOfMemory` instead of pushing the PC into swap.
    pub fn set_kdf_memory_limit(&self, max_bytes: Option<u64>) {
        let value = max_bytes.map_or(NO_LIMIT, |bytes| bytes.min(NO_LIMIT - 1));
        self.kdf_limit.store(value, Ordering::SeqCst);
    }

    /// Cached state only: no I/O, never runs the KDF.
    pub fn status(&self) -> VaultStatus {
        let Ok(inner) = self.inner() else {
            return VaultStatus {
                state: VaultState::Locked,
                kdf: None,
                kdf_upgrade_pending: false,
                failed_attempts: 0,
                retry_after: None,
                foreign_items: 0,
                entry_count: None,
                session_files: 0,
            };
        };
        let unlocked = inner.unlocked.as_ref();
        let state = match (&unlocked, inner.exists) {
            (Some(_), _) => VaultState::Unlocked,
            (None, true) => VaultState::Locked,
            (None, false) => VaultState::Uninitialized,
        };
        let kdf = unlocked
            .map(|u| u.header.kdf)
            .or(inner.header.as_ref().map(|h| h.kdf));
        let target = self.config.kdf;
        VaultStatus {
            state,
            kdf,
            kdf_upgrade_pending: kdf
                .is_some_and(|k| k.is_weaker_than(target) && target.validate_for_create().is_ok()),
            failed_attempts: inner.throttle.failures(),
            retry_after: inner.throttle.retry_after(Instant::now()),
            foreign_items: inner.foreign,
            entry_count: unlocked.map(|u| u.index.entries.len()),
            session_files: unlocked.map_or(inner.leftovers, |u| u.session.files.len()),
        }
    }

    /// Creates a new vault with `password` and leaves it unlocked.
    pub fn create(&self, password: &SecretString) -> Result<(), VaultError> {
        let unlock_gate = gate(&self.unlock_gate);
        {
            let mut inner = self.inner()?;
            let headers = store::read_headers(&self.layout);
            if inner.unlocked.is_some() || headers.exists {
                inner.exists = true;
                inner.header = headers.winner.ok();
                return Err(VaultError::AlreadyExists);
            }
        }
        let normalized = kdf::normalize_password(password)?;
        let params = self.config.kdf.validate_for_create()?;
        let vault_id: [u8; 16] = crypto::random()?;
        let key = crypto::random_key()?;
        let salt = crypto::random()?;
        let kek = kdf::derive_kek_limited(normalized.as_bytes(), &salt, params, self.kdf_limit())?;
        drop(normalized);
        let header = seal_header(&kek, &key, 1, vault_id, params, salt)?;
        drop(kek);

        // Index first, header last, slot A renamed into place: a crash at any point leaves
        // either no header (create works again) or a complete one.
        fs::create_dir_all(&self.layout.files).map_err(VaultError::io("creating the vault"))?;
        store::remove_create_leftovers(&self.layout)?;
        let index = Index::default();
        // Generation 1 goes to slot B so that, normally, generation g sits in slot g % 2.
        let index_slot = 1;
        store::commit_index(&self.layout, &key, &vault_id, 1, &index, index_slot)?;
        store::write_first_headers(&self.layout, &header)?;
        let session = Session::new()?;
        {
            let mut inner = self.inner()?;
            inner.exists = true;
            inner.header = Some(header.clone());
            inner.unlocked = Some(Unlocked {
                key: Arc::new(key),
                vault_id,
                header,
                generation: 1,
                index_slot,
                index,
                session,
            });
            self.epoch.fetch_add(1, Ordering::SeqCst);
        }
        drop(unlock_gate);
        self.ingest_foreign();
        Ok(())
    }

    /// Derives the KEK from `password` and unwraps the vault key with `header`.
    fn open_key(
        &self,
        password: &SecretString,
        header: &HeaderSlot,
    ) -> Result<(Zeroizing<String>, SecretKey), VaultError> {
        // A password outside the policy cannot be the right one; no KDF needed to say so.
        let normalized =
            kdf::normalize_password(password).map_err(|_| VaultError::WrongPassword)?;
        let kek = kdf::derive_kek_limited(
            normalized.as_bytes(),
            &header.salt,
            header.kdf,
            self.kdf_limit(),
        )?;
        let key = crypto::unwrap_vault_key(
            &kek,
            &header.wrap_nonce,
            &header.wrapped_vk,
            &header.wrap_aad(),
        )?;
        Ok((normalized, key))
    }

    /// Runs `open_key` under the throttle: the attempt is recorded before the KDF and resolved
    /// after it. Wrong-password-free errors (no memory, damaged data) do not count.
    fn verify_password(
        &self,
        password: &SecretString,
        header: &HeaderSlot,
    ) -> Result<(Zeroizing<String>, SecretKey), VaultError> {
        let attempt = {
            let mut inner = self.inner()?;
            inner
                .throttle
                .begin(Instant::now(), SystemTime::now())
                .map_err(|retry_after| VaultError::Throttled { retry_after })?
        };
        let verdict = self.open_key(password, header);
        let mut inner = self.inner()?;
        match verdict {
            Ok(opened) => {
                inner.throttle.succeed(attempt, SystemTime::now());
                Ok(opened)
            }
            Err(VaultError::WrongPassword) => {
                inner.throttle.fail(attempt, Instant::now());
                Err(VaultError::WrongPassword)
            }
            Err(other) => {
                inner.throttle.abandon(attempt, SystemTime::now());
                Err(other)
            }
        }
    }

    /// Unlocks with `password`; then recovers from any crash (heals slots, removes orphaned
    /// blobs), upgrades a weak KDF, and encrypts plain files found in the vault folder.
    pub fn unlock(&self, password: &SecretString) -> Result<(), VaultError> {
        let unlock_gate = gate(&self.unlock_gate);
        let store::HeaderRead {
            exists,
            winner,
            stale,
        } = {
            let mut inner = self.inner()?;
            if inner.unlocked.is_some() {
                return Err(VaultError::AlreadyUnlocked);
            }
            let read = store::read_headers(&self.layout);
            inner.exists = read.exists;
            inner.header = read.winner.as_ref().ok().cloned();
            read
        };
        if !exists {
            return Err(VaultError::Uninitialized);
        }
        let header = winner?;
        let (normalized, key) = self.verify_password(password, &header)?;
        let loaded = store::load_index(&self.layout, &key, &header.vault_id)?;
        store::heal_headers(&self.layout, &header, stale);
        store::collect_garbage(&self.layout, &loaded.index);
        let header = self.upgrade_kdf(&normalized, header, &key);
        drop(normalized);
        let session = Session::new()?;
        {
            let mut inner = self.inner()?;
            inner.header = Some(header.clone());
            inner.unlocked = Some(Unlocked {
                key: Arc::new(key),
                vault_id: header.vault_id,
                header,
                generation: loaded.generation,
                index_slot: loaded.slot,
                index: loaded.index,
                session,
            });
            self.epoch.fetch_add(1, Ordering::SeqCst);
        }
        drop(unlock_gate);
        self.ingest_foreign();
        Ok(())
    }

    /// Re-wraps the key with stronger KDF parameters while the password is at hand. Best effort:
    /// on any failure the old header stays and `kdf_upgrade_pending` remains true.
    fn upgrade_kdf(&self, normalized: &str, header: HeaderSlot, key: &SecretKey) -> HeaderSlot {
        let target = self.config.kdf;
        if target.validate_for_create().is_err() || !header.kdf.is_weaker_than(target) {
            return header;
        }
        let params = header.kdf.upgraded_to(target);
        let attempt = || -> Result<HeaderSlot, VaultError> {
            let salt = crypto::random()?;
            let kek =
                kdf::derive_kek_limited(normalized.as_bytes(), &salt, params, self.kdf_limit())?;
            let next = seal_header(
                &kek,
                key,
                header.generation + 1,
                header.vault_id,
                params,
                salt,
            )?;
            store::write_headers(&self.layout, &next)?;
            Ok(next)
        };
        match attempt() {
            Ok(next) => next,
            // A half-written pair may already hold the new slot; report what is on disk.
            Err(_) => store::read_headers(&self.layout)
                .winner
                .ok()
                .filter(|slot| slot.vault_id == header.vault_id)
                .unwrap_or(header),
        }
    }

    /// Verifies `current` (counts toward the throttle) and re-wraps the vault key under `new`.
    /// Only the two header slots change; no file is re-encrypted.
    pub fn change_password(
        &self,
        current: &SecretString,
        new: &SecretString,
    ) -> Result<(), VaultError> {
        let _unlock_gate = gate(&self.unlock_gate);
        let header = {
            let inner = self.inner()?;
            let unlocked = inner
                .unlocked
                .as_ref()
                .ok_or_else(|| not_unlocked(&inner))?;
            unlocked.header.clone()
        };
        let new_normalized = kdf::normalize_password(new)?;
        let (current_normalized, key) = self.verify_password(current, &header)?;
        drop(current_normalized);
        let params = match self.config.kdf.validate_for_create() {
            Ok(target) => header.kdf.upgraded_to(target),
            Err(_) => header.kdf,
        };
        let salt = crypto::random()?;
        let kek =
            kdf::derive_kek_limited(new_normalized.as_bytes(), &salt, params, self.kdf_limit())?;
        drop(new_normalized);
        let next = seal_header(
            &kek,
            &key,
            header.generation + 1,
            header.vault_id,
            params,
            salt,
        )?;
        drop(kek);
        if let Err(e) = store::write_headers(&self.layout, &next) {
            // Slot A may hold the new wrap while slot B failed: then the new password is
            // the one that unlocks, so report what the disk says, not the error.
            let on_disk = store::read_headers(&self.layout).winner;
            if !on_disk.is_ok_and(|winner| winner == next) {
                return Err(e);
            }
        }
        let mut inner = self.inner()?;
        inner.header = Some(next.clone());
        if let Some(unlocked) = inner.unlocked.as_mut() {
            unlocked.header = next;
        }
        Ok(())
    }

    /// Syncs edits from the session folder, drops the key and the index, and wipes the session
    /// folder. With [`LockPolicy::SyncThenWipe`] an edit that cannot be saved aborts the lock
    /// (`UnsyncedEdits`) and the vault stays unlocked; [`LockPolicy::Force`] reports it and locks.
    pub fn lock(&self, policy: LockPolicy) -> Result<LockReport, VaultError> {
        let _unlock_gate = gate(&self.unlock_gate);
        {
            let inner = self.inner()?;
            if inner.unlocked.is_none() {
                return Err(not_unlocked(&inner));
            }
        }
        // Cancel in-flight transfers (including a running sync) before waiting for them.
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let sync_gate = gate(&self.sync_gate);
        let synced = self.sync_pass(true);
        let (mut synced_paths, unsynced) = match synced {
            Ok(report) => {
                let mut paths = report.updated;
                paths.extend(report.imported_new);
                (
                    paths,
                    report
                        .failed
                        .into_iter()
                        .map(|(p, _)| p)
                        .collect::<Vec<_>>(),
                )
            }
            Err(e) if policy == LockPolicy::SyncThenWipe => return Err(e),
            Err(_) => (Vec::new(), self.session_entries()),
        };
        if policy == LockPolicy::SyncThenWipe && !unsynced.is_empty() {
            return Err(VaultError::UnsyncedEdits(unsynced));
        }
        let unlocked = {
            let mut inner = self.inner()?;
            let unlocked = inner.unlocked.take();
            self.epoch.fetch_add(1, Ordering::SeqCst);
            unlocked
        };
        drop(unlocked); // the key and the index names are wiped here
        drop(sync_gate);
        let wipe = session::wipe_sessions(&self.config.session_root);
        let mut inner = self.inner()?;
        inner.leftovers = wipe.left.len();
        inner.foreign = store::foreign_items(&self.layout).len();
        synced_paths.sort();
        synced_paths.dedup();
        Ok(LockReport {
            synced: synced_paths,
            unsynced,
            wiped_files: wipe.wiped,
            undeletable: wipe.left,
        })
    }

    /// Entries that currently have a session copy.
    fn session_entries(&self) -> Vec<crate::VaultPath> {
        self.inner()
            .ok()
            .and_then(|inner| {
                inner
                    .unlocked
                    .as_ref()
                    .map(|u| u.session.files.iter().map(|f| f.entry.clone()).collect())
            })
            .unwrap_or_default()
    }

    /// Retries wiping session files a lock could not delete (for example a file another program
    /// still held open). Only while not unlocked; returns what is still left.
    pub fn wipe_session_leftovers(&self) -> Result<StartupReport, VaultError> {
        let _unlock_gate = gate(&self.unlock_gate);
        let mut inner = self.inner()?;
        if inner.unlocked.is_some() {
            return Err(VaultError::AlreadyUnlocked);
        }
        let wipe = session::wipe_sessions(&self.config.session_root);
        inner.leftovers = wipe.left.len();
        Ok(startup_report(wipe))
    }
}

/// Session files that could not be removed, then entries of the session root that are not
/// session folders (left untouched).
fn startup_report(wipe: session::WipeReport) -> StartupReport {
    let mut left = wipe.left;
    left.extend(wipe.foreign);
    StartupReport {
        stale_files_wiped: wipe.wiped,
        stale_files_left: left,
    }
}

impl fmt::Debug for Vault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match self.inner.try_lock() {
            Ok(inner) if inner.unlocked.is_some() => "unlocked",
            Ok(inner) if inner.exists => "locked",
            Ok(_) => "uninitialized",
            Err(_) => "busy",
        };
        f.debug_struct("Vault")
            .field("state", &state)
            .finish_non_exhaustive()
    }
}

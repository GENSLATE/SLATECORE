//! The launcher's vault: one [`Vault`] plus the auto-lock bookkeeping, with no Tauri types so it
//! is tested on any host. `vault_commands` wraps each method in an async command that runs it on
//! a blocking thread.
//!
//! Start-up opens the vault before any window exists. `Vault::open` takes the vault folder's
//! exclusive lock and then wipes every stale session folder (what a crash or a pulled drive
//! left behind); if another launcher holds the lock it touches nothing.
//!
//! Passwords arrive as `SecretString`, are lent to the vault and dropped (and zeroised) as soon
//! as the call returns. They are never logged, formatted or stored here.

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use genslate_paths::AppPaths;
use genslate_vault::{
    LockPolicy, LockReport, SecretString, StartupReport, SyncReport, Vault, VaultConfig,
    VaultError, VaultPath, VaultState,
};

use crate::auto_lock::{AutoLock, LockReason, Timeouts};
use crate::storage_path::resolve_user_path;
use crate::vault_dto::{
    ConflictDto, EntryDto, LockReportDto, VaultStatusDto, VerifyReportDto, relative_to,
};

/// How long the exit lock may try to save edits before it gives up and wipes anyway.
pub const EXIT_SAVE_BOUND: Duration = Duration::from_secs(10);

/// A file decrypted into the session folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFile {
    /// Where to open it from (never sent to the webview).
    pub absolute: PathBuf,
    /// The same, relative to the session folder, for the webview.
    pub relative: String,
}

/// What an auto-lock check did.
#[derive(Debug)]
pub enum Tick {
    /// Nothing to do (locked, or not yet due).
    Idle,
    /// The vault locked itself.
    Locked(LockReason, LockReportDto),
    /// The vault is due to lock but could not save an edit; it stays unlocked and is retried.
    Blocked(LockReason, VaultError),
}

/// The vault and its timers. Share it as `Arc<VaultService>`.
#[derive(Debug)]
pub struct VaultService {
    vault: Arc<Vault>,
    users_root: PathBuf,
    session_root: PathBuf,
    auto_lock: Mutex<AutoLock>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl VaultService {
    /// Opens the vault of `paths` and wipes stale session folders.
    ///
    /// # Errors
    ///
    /// [`VaultError::Busy`] when another launcher holds the vault (nothing is wiped then),
    /// or an I/O error from the vault and session folders.
    pub fn open(paths: &AppPaths) -> Result<(Self, StartupReport), VaultError> {
        let config = VaultConfig::new(paths.vault_dir(), paths.vault_session_dir());
        let (vault, report) = Vault::open(config)?;
        let service = Self {
            vault: Arc::new(vault),
            users_root: paths.layout.storage.join("users"),
            session_root: paths.vault_session_dir(),
            auto_lock: Mutex::new(AutoLock::new(Instant::now())),
        };
        Ok((service, report))
    }

    /// The vault's state; no I/O, no key derivation.
    pub fn status(&self) -> VaultStatusDto {
        VaultStatusDto::from(&self.vault.status())
    }

    fn touch(&self) {
        locked(&self.auto_lock).touch(Instant::now());
    }

    /// The launcher window was shown or hidden (the hide timer runs while it is hidden).
    pub fn set_visible(&self, visible: bool) {
        locked(&self.auto_lock).set_visible(visible, Instant::now());
    }

    /// Creates the vault with `password` and leaves it unlocked.
    ///
    /// # Errors
    ///
    /// The vault's error (`PasswordRejected`, `AlreadyExists`, ...).
    pub fn create(&self, password: &SecretString) -> Result<VaultStatusDto, VaultError> {
        self.touch();
        self.vault.create(password)?;
        Ok(self.status())
    }

    /// Unlocks with `password`; wrong passwords are rate limited by the vault.
    ///
    /// # Errors
    ///
    /// The vault's error (`WrongPassword`, `Throttled`, ...).
    pub fn unlock(&self, password: &SecretString) -> Result<VaultStatusDto, VaultError> {
        self.touch();
        self.vault.unlock(password)?;
        Ok(self.status())
    }

    /// Locks. Without `force` an edit that cannot be saved aborts the lock (`UnsyncedEdits`).
    ///
    /// # Errors
    ///
    /// The vault's error.
    pub fn lock(&self, force: bool) -> Result<LockReportDto, VaultError> {
        let policy = if force {
            LockPolicy::Force
        } else {
            LockPolicy::SyncThenWipe
        };
        let report = self.vault.lock(policy)?;
        Ok(LockReportDto::new(&report, &self.session_root))
    }

    /// Re-wraps the vault key under `new` after checking `current`.
    ///
    /// # Errors
    ///
    /// The vault's error.
    pub fn change_password(
        &self,
        current: &SecretString,
        new: &SecretString,
    ) -> Result<VaultStatusDto, VaultError> {
        self.touch();
        self.vault.change_password(current, new)?;
        Ok(self.status())
    }

    /// Lists the entries of `dir` (`""` is the root).
    ///
    /// # Errors
    ///
    /// The vault's error (`Locked`, `NotFound`, `InvalidPath`).
    pub fn list(&self, dir: &str) -> Result<Vec<EntryDto>, VaultError> {
        self.touch();
        let entries = self.vault.list(&VaultPath::parse(dir)?)?;
        Ok(entries.iter().map(EntryDto::from).collect())
    }

    /// Encrypts files and folders named relative to `storage/users` into `dest_dir`. The
    /// originals stay where they are.
    ///
    /// # Errors
    ///
    /// `InvalidPath` for a source that is not inside `storage/users`, else the vault's error.
    pub fn import(
        &self,
        sources: &[String],
        dest_dir: &str,
        on_conflict: ConflictDto,
    ) -> Result<Vec<EntryDto>, VaultError> {
        self.touch();
        let dest = VaultPath::parse(dest_dir)?;
        // Check every name before encrypting anything.
        let sources = sources
            .iter()
            .map(|source| resolve_user_path(&self.users_root, source, true))
            .collect::<Result<Vec<_>, _>>()?;
        let mut imported = Vec::with_capacity(sources.len());
        for source in &sources {
            let entry = self.vault.import(source, &dest, on_conflict.into(), None)?;
            imported.push(EntryDto::from(&entry));
        }
        Ok(imported)
    }

    /// Decrypts `path` to `dest_path`, named relative to `storage/users`.
    ///
    /// # Errors
    ///
    /// `InvalidPath` for a destination outside `storage/users`, else the vault's error.
    pub fn export(&self, path: &str, dest_path: &str) -> Result<(), VaultError> {
        self.touch();
        let from = VaultPath::parse(path)?;
        let dest = resolve_user_path(&self.users_root, dest_path, false)?;
        self.vault.export(&from, &dest, None)
    }

    /// Decrypts `path` into the session folder and returns where.
    ///
    /// # Errors
    ///
    /// The vault's error; `Internal` if the vault answered with a path outside the session
    /// folder.
    pub fn open_entry(&self, path: &str) -> Result<SessionFile, VaultError> {
        self.touch();
        let absolute = self.vault.open_in_session(&VaultPath::parse(path)?, None)?;
        if !absolute.starts_with(&self.session_root) {
            return Err(VaultError::Internal(
                "session file outside the session folder",
            ));
        }
        Ok(SessionFile {
            relative: relative_to(&absolute, &self.session_root),
            absolute,
        })
    }

    /// Checks every file of the vault.
    ///
    /// # Errors
    ///
    /// The vault's error.
    pub fn verify(&self) -> Result<VerifyReportDto, VaultError> {
        self.touch();
        let report = self.vault.verify(None)?;
        Ok(VerifyReportDto::from(&report))
    }

    /// Locks the vault if a timer ran out. Called every few seconds.
    pub fn tick(&self, now: Instant, timeouts: Timeouts) -> Tick {
        if self.vault.status().state != VaultState::Unlocked {
            return Tick::Idle;
        }
        let Some(reason) = locked(&self.auto_lock).due(now, timeouts) else {
            return Tick::Idle;
        };
        match self.vault.lock(LockPolicy::SyncThenWipe) {
            Ok(report) => Tick::Locked(reason, LockReportDto::new(&report, &self.session_root)),
            // Someone locked it meanwhile: nothing left to do.
            Err(VaultError::Locked) => Tick::Idle,
            Err(error) => Tick::Blocked(reason, error),
        }
    }

    /// Saves edits made to opened files back into the vault (they are saved on lock too).
    /// `None` when there was nothing to do.
    pub fn sync(&self) -> Option<Result<SyncReport, VaultError>> {
        let status = self.vault.status();
        (status.state == VaultState::Unlocked && status.session_files > 0)
            .then(|| self.vault.sync_session())
    }

    /// Locks for good when the launcher exits: first with a bounded attempt to save every edit,
    /// then, if that fails or takes too long, wiping anyway. Returns what the lock reported, or
    /// `None` when the vault was not unlocked.
    pub fn lock_for_exit(&self, bound: Duration) -> Option<LockReport> {
        if self.vault.status().state != VaultState::Unlocked {
            return None;
        }
        let (done, result) = mpsc::channel();
        let vault = Arc::clone(&self.vault);
        let attempt = thread::Builder::new()
            .name("vault-exit-lock".to_owned())
            .spawn(move || {
                // The receiver is gone only after the bound; a late answer is not needed then.
                let _ = done.send(vault.lock(LockPolicy::SyncThenWipe));
            });
        if attempt.is_ok()
            && let Ok(Ok(report)) = result.recv_timeout(bound)
        {
            return Some(report);
        }
        self.vault.lock(LockPolicy::Force).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_paths::{Environment, resolve_with};
    use genslate_testing::{TempTree, fake_suite};
    use std::error::Error;
    use std::fs;

    type TestResult = Result<(), Box<dyn Error>>;

    fn paths_in(tmp: &TempTree) -> Result<AppPaths, Box<dyn Error>> {
        let root = fake_suite(tmp.path())?;
        let env = Environment {
            install_dir_override: Some(root),
            ..Environment::default()
        };
        let paths = resolve_with("launcher", &env)?;
        paths.create_dirs()?;
        Ok(paths)
    }

    #[test]
    fn startup_wipes_stale_vault_session_dir_before_first_window() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        // What a crash or a pulled drive leaves: a decrypted file in a session folder.
        let stale = paths.vault_session_dir().join("deadbeef");
        fs::create_dir_all(&stale)?;
        fs::write(stale.join("leak.txt"), "plaintext")?;

        let (service, report) = VaultService::open(&paths)?;

        assert!(report.stale_files_wiped >= 1, "{report:?}");
        assert!(!stale.join("leak.txt").exists(), "the stale file is gone");
        assert!(report.stale_files_left.is_empty(), "{report:?}");
        assert_eq!(service.status().session_files, 0);
        Ok(())
    }

    #[test]
    fn a_second_launcher_on_the_drive_wipes_nothing() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let (_first, _) = VaultService::open(&paths)?;
        let live = paths.vault_session_dir().join("cafe0001");
        fs::create_dir_all(&live)?;
        fs::write(live.join("open.txt"), "in use by the first launcher")?;

        let second = VaultService::open(&paths);
        assert!(matches!(second, Err(VaultError::Busy)), "{second:?}");
        assert!(
            live.join("open.txt").is_file(),
            "the first launcher's session survives"
        );
        Ok(())
    }

    #[test]
    fn a_new_vault_reports_uninitialized_and_locked_commands_refuse() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let (service, _) = VaultService::open(&paths)?;
        let json = serde_json::to_value(service.status())?;
        assert_eq!(json["state"], "uninitialized");
        assert!(matches!(
            service.list(""),
            Err(VaultError::Uninitialized | VaultError::Locked)
        ));
        assert!(service.lock(false).is_err());
        assert!(service.sync().is_none());
        assert!(service.lock_for_exit(Duration::from_secs(1)).is_none());
        Ok(())
    }

    #[test]
    fn import_and_export_refuse_paths_outside_storage_users() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let (service, _) = VaultService::open(&paths)?;
        let bad = ["../vault/vault.a.gvh".to_owned()];
        let result = service.import(&bad, "", ConflictDto::Fail);
        assert!(
            matches!(result, Err(VaultError::InvalidPath(_))),
            "{result:?}"
        );
        let result = service.export("a.txt", "../../outside.txt");
        assert!(
            matches!(result, Err(VaultError::InvalidPath(_))),
            "{result:?}"
        );
        Ok(())
    }

    #[test]
    fn create_unlock_import_open_and_lock_wipes_the_session() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let docs = paths.layout.storage.join("users/shared/Documents");
        fs::create_dir_all(&docs)?;
        fs::write(docs.join("notes.txt"), "hello")?;
        let (service, _) = VaultService::open(&paths)?;
        let password = SecretString::from("correct horse battery");

        let created = service.create(&password)?;
        assert_eq!(serde_json::to_value(&created)?["state"], "unlocked");
        let imported = service.import(
            &["shared/Documents/notes.txt".to_owned()],
            "",
            ConflictDto::Fail,
        )?;
        assert_eq!(imported.len(), 1);
        let listed = service.list("")?;
        assert_eq!(listed.len(), 1);

        let opened = service.open_entry("notes.txt")?;
        assert_eq!(fs::read_to_string(&opened.absolute)?, "hello");
        assert!(!opened.relative.contains(':'), "{}", opened.relative);
        assert_eq!(service.status().session_files, 1);

        let report = service.lock(false)?;
        assert!(report.wiped_files >= 1, "{report:?}");
        assert!(!opened.absolute.exists(), "no plaintext left after lock");
        assert!(matches!(service.list(""), Err(VaultError::Locked)));

        let wrong = service.unlock(&SecretString::from("wrong password!!"));
        assert!(matches!(wrong, Err(VaultError::WrongPassword)));
        service.unlock(&password)?;
        assert_eq!(service.list("")?.len(), 1);
        Ok(())
    }

    #[test]
    fn the_exit_lock_wipes_an_unlocked_vault() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let (service, _) = VaultService::open(&paths)?;
        service.create(&SecretString::from("correct horse battery"))?;
        assert!(service.lock_for_exit(EXIT_SAVE_BOUND).is_some());
        let json = serde_json::to_value(service.status())?;
        assert_eq!(json["state"], "locked");
        Ok(())
    }

    #[test]
    fn the_idle_and_hidden_timers_lock_a_vault_that_is_open() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let (service, _) = VaultService::open(&paths)?;
        service.create(&SecretString::from("correct horse battery"))?;
        let timeouts = Timeouts::from_minutes(10, 5);
        let start = Instant::now();

        service.set_visible(true);
        assert!(matches!(service.tick(start, timeouts), Tick::Idle));
        let later = start + Duration::from_mins(11);
        match service.tick(later, timeouts) {
            Tick::Locked(reason, _) => assert_eq!(reason, LockReason::Idle),
            other => return Err(format!("expected a lock, got {other:?}").into()),
        }
        assert!(
            matches!(service.tick(later, timeouts), Tick::Idle),
            "already locked"
        );
        Ok(())
    }
}

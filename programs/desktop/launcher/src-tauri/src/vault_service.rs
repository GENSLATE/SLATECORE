//! The launcher's vault: one [`Vault`] plus the auto-lock bookkeeping, with no Tauri types so it
//! is tested on any host. `vault_commands` wraps each method in an async command that runs it on
//! a blocking thread.
//!
//! Start-up opens the vault before any window exists. `Vault::open` takes the vault folder's
//! exclusive lock and then wipes every stale session folder (what a crash or a pulled drive
//! left behind); if another launcher holds the lock it touches nothing.
//!
//! Passwords arrive as `SecretString`, are lent to the vault and dropped (and zeroised) as soon
//! as the call returns. They are never logged, formatted or stored here. Limit: before they reach
//! this module the password crossed the IPC as text in the request body and went through
//! `serde_json::Value` on its way into the `SecretString`; those intermediate copies are plain
//! `String`s that are freed but not zeroised. Closing that needs a deserializer that builds the
//! secret straight from the request bytes (Tauri does not offer one for command arguments).
//!
//! Exit: [`VaultService::lock_for_exit`] gives the save-then-wipe lock a bounded time and, if it
//! does not finish, leaves the process to exit without waiting (see [`ExitLock::TimedOut`]).

use std::path::{Path, PathBuf};
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
use crate::storage_path::{resolve_again, resolve_user_path, verify_written};
use crate::vault_dto::{
    ConflictDto, EntryDto, LockReportDto, VaultStatusDto, VerifyReportDto, relative_to,
};

/// How long the exit lock may try to save edits before it gives up and wipes anyway.
pub const EXIT_SAVE_BOUND: Duration = Duration::from_secs(10);

/// What the lock at exit did.
#[derive(Debug)]
pub enum ExitLock {
    /// The vault was not unlocked: nothing to do.
    NotUnlocked,
    /// Edits were saved and the session folder wiped.
    Locked(LockReport),
    /// Saving did not work (the report lists the files), so the vault was locked and the session
    /// folder wiped anyway: unsaved edits are lost.
    Forced(LockReport),
    /// The save-then-wipe lock did not finish within the bound (a stuck file system call holds
    /// the vault's gates, so even a forced lock would wait behind it). The launcher exits
    /// without waiting; the decrypted files stay on the drive until the next start, whose
    /// start-up wipe removes every session folder before anything else runs.
    TimedOut,
    /// Neither lock worked.
    Failed(VaultError),
}

/// The outcome of [`run_bounded`].
#[derive(Debug, PartialEq, Eq)]
pub enum Bounded<T> {
    /// The work finished in time.
    Done(T),
    /// It was still running when the bound ran out. It keeps running on its own thread; the
    /// caller must not wait for it again.
    TimedOut,
    /// The thread could not be started.
    NotStarted,
}

/// Runs `work` on a thread of its own and waits at most `bound` for its answer.
pub fn run_bounded<T, F>(bound: Duration, work: F) -> Bounded<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (done, result) = mpsc::channel();
    let started = thread::Builder::new()
        .name("vault-bounded".to_owned())
        .spawn(move || {
            // The receiver is gone after the bound; a late answer is not needed then.
            let _ = done.send(work());
        });
    if started.is_err() {
        return Bounded::NotStarted;
    }
    result
        .recv_timeout(bound)
        .map_or(Bounded::TimedOut, Bounded::Done)
}

/// The exit policy, apart from the vault so it can be tested: try `attempt` (save, then wipe)
/// for at most `bound`; if it ended in an error, or could not start, run `force` (wipe whatever
/// the cost); if it is still running at the bound, run nothing else, because `force` would
/// queue behind the same locks.
pub fn exit_lock_with<A, F>(bound: Duration, attempt: A, force: F) -> ExitLock
where
    A: FnOnce() -> Result<LockReport, VaultError> + Send + 'static,
    F: FnOnce() -> Result<LockReport, VaultError>,
{
    match run_bounded(bound, attempt) {
        Bounded::Done(Ok(report)) => ExitLock::Locked(report),
        // Locked by someone else in the meantime: nothing left to do.
        Bounded::Done(Err(VaultError::Locked)) => ExitLock::NotUnlocked,
        Bounded::TimedOut => ExitLock::TimedOut,
        Bounded::Done(Err(_)) | Bounded::NotStarted => match force() {
            Ok(report) => ExitLock::Forced(report),
            Err(error) => ExitLock::Failed(error),
        },
    }
}

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
        let names = sources;
        let sources = names
            .iter()
            .map(|source| resolve_user_path(&self.users_root, source, true))
            .collect::<Result<Vec<_>, _>>()?;
        let mut imported = Vec::with_capacity(sources.len());
        for (input, source) in names.iter().zip(&sources) {
            // Just before this file's turn (earlier files may have taken a while): is it still
            // the file that was checked?
            let source = resolve_again(&self.users_root, input, source)?;
            let entry = self
                .vault
                .import(&source, &dest, on_conflict.into(), None)
                .map_err(|error| without_source_path(error, input))?;
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
        self.vault.export(&from, &dest, None)?;
        // The vault decrypts and renames inside its own crate, which cannot be interrupted from
        // here: look again at where the file landed (best effort, see `verify_written`).
        verify_written(&self.users_root, &dest)
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

    /// Locks for good when the launcher exits: first with a bounded attempt to save every edit
    /// and wipe; a failed attempt is followed by a forced lock, a stuck one is left alone (see
    /// [`ExitLock`]).
    pub fn lock_for_exit(&self, bound: Duration) -> ExitLock {
        if self.vault.status().state != VaultState::Unlocked {
            return ExitLock::NotUnlocked;
        }
        let attempt = Arc::clone(&self.vault);
        let force = Arc::clone(&self.vault);
        exit_lock_with(
            bound,
            move || attempt.lock(LockPolicy::SyncThenWipe),
            move || force.lock(LockPolicy::Force),
        )
    }
}

/// An import error that names the source file by its absolute path (the vault's own `NotFound`
/// for a file that vanished) is rewritten to name the relative `input` the webview sent, so no
/// absolute path, and with it no drive letter or folder, reaches the webview.
fn without_source_path(error: VaultError, input: &str) -> VaultError {
    match error {
        VaultError::NotFound(text) if Path::new(&text).is_absolute() => {
            VaultError::NotFound(input.to_owned())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_paths::{Environment, resolve_with};
    use genslate_testing::{TempTree, fake_suite};
    use std::error::Error;
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};

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
        assert!(matches!(
            service.lock_for_exit(Duration::from_secs(1)),
            ExitLock::NotUnlocked
        ));
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
        assert!(matches!(
            service.lock_for_exit(EXIT_SAVE_BOUND),
            ExitLock::Locked(_)
        ));
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

    #[test]
    fn an_export_lands_inside_users_and_passes_the_after_check() -> TestResult {
        let tmp = TempTree::new()?;
        let paths = paths_in(&tmp)?;
        let docs = paths.layout.storage.join("users/shared/Documents");
        fs::create_dir_all(&docs)?;
        fs::create_dir_all(paths.layout.storage.join("users/shared/Desktop"))?;
        fs::write(docs.join("notes.txt"), "hello")?;
        let (service, _) = VaultService::open(&paths)?;
        service.create(&SecretString::from("correct horse battery"))?;
        service.import(
            &["shared/Documents/notes.txt".to_owned()],
            "",
            ConflictDto::Fail,
        )?;

        service.export("notes.txt", "shared/Desktop/out.txt")?;

        let written = paths.layout.storage.join("users/shared/Desktop/out.txt");
        assert_eq!(fs::read_to_string(written)?, "hello");
        Ok(())
    }

    fn report() -> LockReport {
        LockReport::default()
    }

    #[test]
    fn a_finished_attempt_is_the_exit_lock_and_nothing_is_forced() {
        let forced = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&forced);
        let outcome = exit_lock_with(
            Duration::from_secs(5),
            || Ok(report()),
            move || {
                flag.store(true, Ordering::SeqCst);
                Ok(report())
            },
        );
        assert!(matches!(outcome, ExitLock::Locked(_)), "{outcome:?}");
        assert!(!forced.load(Ordering::SeqCst));
    }

    #[test]
    fn a_failed_attempt_is_followed_by_a_forced_lock() {
        let outcome = exit_lock_with(
            Duration::from_secs(5),
            || Err(VaultError::UnsyncedEdits(Vec::new())),
            || Ok(report()),
        );
        assert!(matches!(outcome, ExitLock::Forced(_)), "{outcome:?}");

        let outcome = exit_lock_with(
            Duration::from_secs(5),
            || Err(VaultError::Cancelled),
            || Err(VaultError::Busy),
        );
        assert!(
            matches!(outcome, ExitLock::Failed(VaultError::Busy)),
            "{outcome:?}"
        );
    }

    #[test]
    fn a_stuck_attempt_is_not_waited_for_and_never_forced() {
        let forced = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&forced);
        let started = Instant::now();
        let outcome = exit_lock_with(
            Duration::from_millis(150),
            || {
                // A file-system call that does not come back while the launcher exits.
                thread::sleep(Duration::from_secs(3));
                Ok(report())
            },
            move || {
                // Would queue behind the same gates as the stuck attempt.
                flag.store(true, Ordering::SeqCst);
                Ok(report())
            },
        );
        assert!(matches!(outcome, ExitLock::TimedOut), "{outcome:?}");
        assert!(
            !forced.load(Ordering::SeqCst),
            "no forced lock behind a stuck one"
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the exit waited {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn run_bounded_returns_the_answer_or_gives_up() {
        assert_eq!(run_bounded(Duration::from_secs(5), || 7), Bounded::Done(7));
        let slow = run_bounded(Duration::from_millis(50), || {
            thread::sleep(Duration::from_millis(600));
            1
        });
        assert_eq!(slow, Bounded::TimedOut);
    }

    #[test]
    fn an_import_error_names_the_relative_input_not_the_absolute_path() {
        let absolute = std::env::temp_dir()
            .join("users")
            .join("shared")
            .join("a.txt");
        let hidden = without_source_path(
            VaultError::NotFound(absolute.display().to_string()),
            "shared/a.txt",
        );
        assert!(
            matches!(&hidden, VaultError::NotFound(text) if text == "shared/a.txt"),
            "{hidden:?}"
        );

        // A vault path (the destination folder) is already relative and is left alone.
        let kept = without_source_path(VaultError::NotFound("Docs/x".to_owned()), "shared/a.txt");
        assert!(
            matches!(&kept, VaultError::NotFound(text) if text == "Docs/x"),
            "{kept:?}"
        );
        assert!(matches!(
            without_source_path(VaultError::Busy, "shared/a.txt"),
            VaultError::Busy
        ));
    }
}

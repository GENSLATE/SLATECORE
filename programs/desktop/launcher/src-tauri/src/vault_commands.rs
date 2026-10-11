//! The vault's IPC commands and the background thread that locks it.
//!
//! Every command is `async` and runs the vault call on a blocking thread: unlocking derives a
//! key with Argon2id (a second or more, 64 MiB and up), listing and importing do file I/O, and
//! none of that may stall the window's event loop. The password arrives once, as a
//! [`SecretString`], is lent to the vault and is dropped (and zeroised) when the call returns.
//! It is never logged, formatted or stored here.
//!
//! Errors go to the webview as [`VaultErrorDto`]: a stable code and a message with no password,
//! no key and no OS path. After any call that can change the vault's state the shell emits
//! `vault://status`, so the UI follows automatic locks and a second window the same way.
//!
//! The vault locks itself after `[vault] idle-lock-minutes` without use (default 10) and
//! `hide-lock-minutes` after the launcher was hidden (default 5), and when the launcher exits
//! (`lib.rs`). Edits to opened files are saved back to the vault every 30 seconds and on lock.
//!
//! An automatic lock never throws away an edit it could not save: it stays blocked, the vault
//! stays unlocked and the shell retries every tick. The UI is told with `vault://status` and
//! `vault://lock-blocked` (see [`LockBlockedDto`]) when a block starts, changes, and then once
//! a minute while it lasts. Whether such a lock should eventually wipe unsaved edits is an open
//! owner decision.
//!
//! `vault_open` hands the decrypted copy to Explorer (see `os_open` for the traces that leaves on
//! the host) and refuses the file types that could run code (`open_policy`, error code
//! `BLOCKED_TYPE`).

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use genslate_vault::{SecretString, VaultError};
use tauri::{AppHandle, Manager};

use crate::auto_lock::{BlockedNotifier, Timeouts};
use crate::events;
use crate::open_policy;
use crate::os_open;
use crate::reload::emit_main;
use crate::state::{Launcher, VaultSlot};
use crate::vault_dto::{
    ConflictDto, EntryDto, LockBlockedDto, LockReportDto, OpenedDto, VaultErrorDto, VaultStatusDto,
    VerifyReportDto,
};
use crate::vault_service::{Tick, VaultService};

type CommandResult<T> = Result<T, VaultErrorDto>;

/// How often the background thread checks the timers.
const TICK: Duration = Duration::from_secs(5);
/// Every this many ticks the opened files are saved back to the vault.
const SYNC_EVERY_TICKS: u32 = 6;

fn service(app: &AppHandle) -> CommandResult<Arc<VaultService>> {
    match &app.state::<Launcher>().vault {
        VaultSlot::Ready(service) => Ok(Arc::clone(service)),
        VaultSlot::Unavailable(error) => Err(error.clone()),
    }
}

/// Runs `work` against the vault on a blocking thread.
async fn blocking<T, F>(app: &AppHandle, work: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&VaultService) -> Result<T, VaultError> + Send + 'static,
{
    let service = service(app)?;
    tauri::async_runtime::spawn_blocking(move || work(&service))
        .await
        .map_err(|_| VaultErrorDto::from(VaultError::Internal("the vault task stopped")))?
        .map_err(VaultErrorDto::from)
}

/// Tells the UI the vault's state, whatever the last call did.
fn emit_status(app: &AppHandle) {
    if let Some(service) = app.state::<Launcher>().vault_service() {
        emit_main(app, events::VAULT_STATUS, service.status());
    }
}

/// Runs `work`, then emits the new status whether it worked or not (a wrong password changes
/// the failed-attempt count).
async fn blocking_then_status<T, F>(app: &AppHandle, work: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&VaultService) -> Result<T, VaultError> + Send + 'static,
{
    let result = blocking(app, work).await;
    emit_status(app);
    result
}

#[tauri::command]
pub async fn vault_status(app: AppHandle) -> CommandResult<VaultStatusDto> {
    // A vault that could not be opened reads as locked and empty; the reason comes with the
    // first command that needs the vault.
    if app.state::<Launcher>().vault_service().is_none() {
        return Ok(VaultStatusDto::unavailable());
    }
    blocking(&app, |service| Ok(service.status())).await
}

#[tauri::command]
pub async fn vault_create(app: AppHandle, password: SecretString) -> CommandResult<VaultStatusDto> {
    blocking_then_status(&app, move |service| service.create(&password)).await
}

#[tauri::command]
pub async fn vault_unlock(app: AppHandle, password: SecretString) -> CommandResult<VaultStatusDto> {
    blocking_then_status(&app, move |service| service.unlock(&password)).await
}

#[tauri::command]
pub async fn vault_lock(app: AppHandle, force: Option<bool>) -> CommandResult<LockReportDto> {
    let force = force.unwrap_or(false);
    blocking_then_status(&app, move |service| service.lock(force)).await
}

#[tauri::command]
pub async fn vault_change_password(
    app: AppHandle,
    current: SecretString,
    next: SecretString,
) -> CommandResult<VaultStatusDto> {
    blocking_then_status(&app, move |service| {
        service.change_password(&current, &next)
    })
    .await
}

#[tauri::command]
pub async fn vault_list(app: AppHandle, dir: String) -> CommandResult<Vec<EntryDto>> {
    blocking(&app, move |service| service.list(&dir)).await
}

#[tauri::command]
pub async fn vault_import(
    app: AppHandle,
    sources: Vec<String>,
    dest_dir: String,
    on_conflict: ConflictDto,
) -> CommandResult<Vec<EntryDto>> {
    blocking_then_status(&app, move |service| {
        service.import(&sources, &dest_dir, on_conflict)
    })
    .await
}

#[tauri::command]
pub async fn vault_export(app: AppHandle, path: String, dest_path: String) -> CommandResult<()> {
    blocking(&app, move |service| service.export(&path, &dest_path)).await
}

#[tauri::command]
pub async fn vault_open(app: AppHandle, path: String) -> CommandResult<OpenedDto> {
    // Before anything is decrypted: a type that could run code is not opened at all.
    if open_policy::is_blocked_type(&path) {
        return Err(VaultErrorDto::blocked_type());
    }
    let (file, blocked, opened) = blocking_then_status(&app, move |service| {
        let file = service.open_entry(&path)?;
        // Once more on the name of the file that is really about to be opened.
        let blocked = file
            .absolute
            .file_name()
            .is_some_and(|name| open_policy::is_blocked_type(&name.to_string_lossy()));
        let opened = !blocked && os_open::open_path(&file.absolute).is_ok();
        Ok((file, blocked, opened))
    })
    .await?;
    if blocked {
        return Err(VaultErrorDto::blocked_type());
    }
    if !opened {
        return Err(VaultErrorDto::open_failed());
    }
    Ok(OpenedDto {
        session_path: file.relative,
    })
}

#[tauri::command]
pub async fn vault_verify(app: AppHandle) -> CommandResult<VerifyReportDto> {
    blocking(&app, VaultService::verify).await
}

/// Starts the thread that locks the vault when a timer runs out and saves edits of opened files.
pub fn spawn_background(app: AppHandle) {
    let spawned = thread::Builder::new()
        .name("vault-auto_lock".to_owned())
        .spawn(move || background(&app));
    if let Err(failure) = spawned {
        log::error!("the vault timer thread failed to start: {failure}");
    }
}

fn background(app: &AppHandle) {
    let mut ticks: u32 = 0;
    let mut blocked = BlockedNotifier::default();
    loop {
        thread::sleep(TICK);
        let launcher = app.state::<Launcher>();
        let Some(service) = launcher.vault_service().cloned() else {
            return;
        };
        let timeouts = {
            let settings = launcher.settings();
            Timeouts::from_minutes(
                settings.config.vault.idle_lock_minutes,
                settings.config.vault.hide_lock_minutes,
            )
        };
        match service.tick(Instant::now(), timeouts) {
            Tick::Idle => blocked.clear(),
            Tick::Locked(reason, report) => {
                blocked.clear();
                log::info!(
                    "the vault locked itself ({reason:?}); {} session files wiped",
                    report.wiped_files
                );
                emit_main(app, events::VAULT_STATUS, service.status());
            }
            Tick::Blocked(reason, error) => {
                let notice = LockBlockedDto::new(reason, &error);
                if blocked.blocked(reason, &notice.cause()) {
                    // The code only: the message can name vault entries.
                    log::warn!(
                        "the vault could not lock itself ({reason:?}): {}; still unlocked, \
                         retrying",
                        notice.error.code
                    );
                    emit_main(app, events::VAULT_LOCK_BLOCKED, notice);
                    emit_main(app, events::VAULT_STATUS, service.status());
                }
            }
        }
        ticks = ticks.wrapping_add(1);
        if ticks.is_multiple_of(SYNC_EVERY_TICKS) {
            sync(app, &service);
        }
    }
}

fn sync(app: &AppHandle, service: &VaultService) {
    match service.sync() {
        None => {}
        Some(Ok(report)) => {
            if !report.failed.is_empty() {
                log::warn!(
                    "{} opened files could not be saved back to the vault",
                    report.failed.len()
                );
            }
            if !report.updated.is_empty() || !report.imported_new.is_empty() {
                emit_main(app, events::VAULT_STATUS, service.status());
            }
        }
        // The code only: a vault error's text can name vault entries.
        Some(Err(error)) => log::warn!(
            "saving opened vault files failed: {}",
            VaultErrorDto::from(&error).code
        ),
    }
}

/// The vault commands must be `async` so they never run on the window's event loop. A command
/// that is a plain `fn` does not return a future and stops this module compiling.
#[cfg(test)]
mod async_signatures {
    use std::future::Future;

    use super::*;

    fn is_async1<A, F: Future>(_: fn(A) -> F) {}
    fn is_async2<A, B, F: Future>(_: fn(A, B) -> F) {}
    fn is_async3<A, B, C, F: Future>(_: fn(A, B, C) -> F) {}
    fn is_async4<A, B, C, D, F: Future>(_: fn(A, B, C, D) -> F) {}

    #[test]
    fn all_ten_vault_commands_are_async_fns() {
        is_async1(vault_status);
        is_async1(vault_verify);
        is_async2(vault_create);
        is_async2(vault_unlock);
        is_async2(vault_lock);
        is_async2(vault_list);
        is_async2(vault_open);
        is_async3(vault_change_password);
        is_async3(vault_export);
        is_async4(vault_import);
    }
}

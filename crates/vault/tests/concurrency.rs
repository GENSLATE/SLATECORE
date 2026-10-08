//! Concurrency: lock during reads, parallel unlock (X1, X2, 8.2 contract).

mod common;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::{Fixture, PASSWORD, TestResult, pseudo_random, pw};
use genslate_vault::{LockPolicy, Vault, VaultError, VaultPath, VaultState};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn vault_and_errors_are_send_and_sync() {
    assert_send_sync::<Vault>();
    assert_send_sync::<VaultError>();
}

#[test]
fn concurrent_lock_during_read_returns_data_or_locked_never_partial() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    let content = pseudo_random(6 * 1024 * 1024 + 3, 99);
    fixture.import_bytes(&vault, "", "big.bin", &content)?;
    fixture.import_bytes(&vault, "", "small.txt", b"small")?;
    let scratch = fixture.scratch()?;
    let outcomes = [AtomicUsize::new(0), AtomicUsize::new(0)]; // [data, locked-or-cancelled]

    for round in 0..3 {
        if round > 0 {
            vault.unlock(&pw(PASSWORD))?;
        }
        std::thread::scope(|scope| -> TestResult {
            let mut readers = Vec::new();
            for reader in 0..3 {
                let vault = &vault;
                let content = &content;
                let scratch = &scratch;
                let outcomes = &outcomes;
                readers.push(scope.spawn(move || -> Result<(), String> {
                    for i in 0..200 {
                        let dest: PathBuf = scratch.join(format!("r{round}-{reader}-{i}"));
                        let part = PathBuf::from(format!("{}.gvpart", dest.display()));
                        match vault.export(
                            &VaultPath::parse("big.bin").map_err(|e| e.to_string())?,
                            &dest,
                            None,
                        ) {
                            Ok(()) => {
                                let got = fs::read(&dest).map_err(|e| e.to_string())?;
                                if got != *content {
                                    return Err(format!(
                                        "partial or wrong data ({} bytes)",
                                        got.len()
                                    ));
                                }
                                fs::remove_file(&dest).map_err(|e| e.to_string())?;
                                outcomes[0].fetch_add(1, Ordering::Relaxed);
                            }
                            Err(VaultError::Locked | VaultError::Cancelled) => {
                                if dest.exists() || part.exists() {
                                    return Err("a cancelled export left a file behind".into());
                                }
                                outcomes[1].fetch_add(1, Ordering::Relaxed);
                                return Ok(());
                            }
                            Err(other) => return Err(format!("unexpected error: {other:?}")),
                        }
                        match vault.list(&VaultPath::root()) {
                            Ok(entries) if entries.len() == 2 => {}
                            Ok(entries) => {
                                return Err(format!("partial listing: {}", entries.len()));
                            }
                            Err(VaultError::Locked) => return Ok(()),
                            Err(other) => return Err(format!("unexpected list error: {other:?}")),
                        }
                    }
                    Err("the vault never locked".into())
                }));
            }
            std::thread::sleep(Duration::from_millis(40 + 30 * round));
            vault.lock(LockPolicy::Force)?;
            for reader in readers {
                reader.join().map_err(|_| "reader panicked")??;
            }
            Ok(())
        })?;
        assert_eq!(vault.status().state, VaultState::Locked);
        assert_eq!(common::count_files(fixture.session_root())?, 0);
    }
    assert!(outcomes[1].load(Ordering::Relaxed) >= 3);
    // No partial blob or index entry was left by anything that was cancelled.
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(fixture.blobs()?.len(), 2);
    common::assert_empty(&vault.verify(None)?.problems);
    Ok(())
}

#[test]
fn lock_cancels_a_running_import_without_leftovers() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    let src = fixture.scratch()?.join("huge.bin");
    fs::write(&src, pseudo_random(48 * 1024 * 1024, 5))?;
    let started = AtomicUsize::new(0);

    let result = std::thread::scope(|scope| {
        let importer = scope.spawn(|| {
            let progress = |done: u64, _total: u64| {
                if done > 0 {
                    started.store(1, Ordering::Relaxed);
                }
            };
            vault.import(
                &src,
                &VaultPath::root(),
                genslate_vault::Conflict::Fail,
                Some(&progress),
            )
        });
        while started.load(Ordering::Relaxed) == 0 && !importer.is_finished() {
            std::thread::yield_now();
        }
        let lock = vault.lock(LockPolicy::Force);
        (importer.join(), lock)
    });
    let (import, lock) = result;
    lock?;
    let import = import.map_err(|_| "importer panicked")?;
    match import {
        Err(VaultError::Cancelled | VaultError::Locked) => {
            vault.unlock(&pw(PASSWORD))?;
            common::assert_empty(&vault.list(&VaultPath::root())?);
            assert!(fixture.blobs()?.is_empty(), "no partial blob");
        }
        // The machine was fast enough to finish first: then the entry must be complete.
        Ok(_) => {
            vault.unlock(&pw(PASSWORD))?;
            common::assert_empty(&vault.verify(None)?.problems);
        }
        Err(other) => return Err(format!("unexpected: {other:?}").into()),
    }
    Ok(())
}

#[test]
fn concurrent_unlock_exactly_one_succeeds() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    let results: Vec<Result<(), VaultError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| vault.unlock(&pw(PASSWORD))))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or(Err(VaultError::Internal("panic"))))
            .collect()
    });
    let ok = results.iter().filter(|r| r.is_ok()).count();
    let already = results
        .iter()
        .filter(|r| matches!(r, Err(VaultError::AlreadyUnlocked)))
        .count();
    assert_eq!((ok, already), (1, 7), "{results:?}");
    assert_eq!(vault.status().failed_attempts, 0);
    Ok(())
}

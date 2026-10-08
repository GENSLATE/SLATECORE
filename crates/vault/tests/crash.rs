//! Crash safety, garbage collection and foreign items (C1 to C4, C7, P6).

mod common;

use std::fs;

use common::{Fixture, OTHER_PASSWORD, PASSWORD, TestResult, pw};
use genslate_vault::format::header::HeaderSlot;
use genslate_vault::{LockPolicy, VaultPath, VaultState};

#[test]
fn crash_between_blob_write_and_index_commit_recovers_previous_index() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"version one")?;
    let index_a = fixture.root().join("index.a.gvi");
    let index_b = fixture.root().join("index.b.gvi");
    let saved_a = fs::read(&index_a)?;
    let saved_b = fs::read(&index_b)?;
    let blobs_before = fixture.blobs()?;

    // A genuine blob reaches the disk; then the "process dies" before the index commit:
    // restore both index slots to their pre-import state.
    fixture.import_bytes(&vault, "", "b.txt", b"never committed")?;
    assert_eq!(fixture.blobs()?.len(), 2);
    vault.lock(LockPolicy::Force)?;
    drop(vault);
    fs::write(&index_a, &saved_a)?;
    fs::write(&index_b, &saved_b)?;

    let vault = fixture.open()?;
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(common::names(&vault, "")?, vec!["a.txt".to_owned()]);
    assert_eq!(fixture.export_bytes(&vault, "a.txt")?, b"version one");
    assert_eq!(
        fixture.blobs()?,
        blobs_before,
        "the orphan blob was garbage-collected"
    );
    Ok(())
}

#[test]
fn torn_index_commit_recovers_previous_generation() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"one")?; // generation 2 -> slot A
    let blobs_before = fixture.blobs()?;
    let index_b = fixture.root().join("index.b.gvi"); // generation 1, overwritten next
    fixture.import_bytes(&vault, "", "b.txt", b"two")?; // generation 3 -> slot B
    vault.lock(LockPolicy::Force)?;
    drop(vault);

    // Tear the generation-3 write in half.
    let full = fs::read(&index_b)?;
    fs::write(&index_b, &full[..full.len() / 2])?;

    let vault = fixture.open()?;
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(common::names(&vault, "")?, vec!["a.txt".to_owned()]);
    assert_eq!(
        fixture.blobs()?,
        blobs_before,
        "b.txt's blob is an orphan and was removed"
    );

    // The vault keeps working: the next commit lands on the healed slot.
    fixture.import_bytes(&vault, "", "c.txt", b"three")?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(
        common::names(&vault, "")?,
        vec!["a.txt".to_owned(), "c.txt".to_owned()]
    );
    Ok(())
}

#[test]
fn crash_after_commit_before_old_blob_delete_is_cleaned() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"old")?;
    let old_blob = fixture.blobs()?.remove(0);
    let old_bytes = fs::read(&old_blob)?;
    let src = fixture.scratch()?.join("a.txt");
    fs::write(&src, b"new")?;
    vault.import(
        &src,
        &VaultPath::root(),
        genslate_vault::Conflict::Replace,
        None,
    )?;
    assert!(
        !old_blob.exists(),
        "a replaced blob is deleted after the commit"
    );
    vault.lock(LockPolicy::SyncThenWipe)?;

    // Simulate the crash window: the superseded blob is still on disk.
    fs::write(&old_blob, &old_bytes)?;
    // Foreign junk inside files/ is never touched by garbage collection.
    let junk = fixture.files_dir().join("notes.txt");
    fs::write(&junk, b"not a blob")?;
    vault.unlock(&pw(PASSWORD))?;
    assert!(!old_blob.exists());
    assert!(junk.exists());
    assert_eq!(fixture.export_bytes(&vault, "a.txt")?, b"new");
    Ok(())
}

#[test]
fn header_rewrite_crash_windows() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    let header_a = fixture.root().join("vault.a.gvh");
    let header_b = fixture.root().join("vault.b.gvh");
    let old = fs::read(&header_b)?;
    vault.unlock(&pw(PASSWORD))?;
    vault.change_password(&pw(PASSWORD), &pw(OTHER_PASSWORD))?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    drop(vault);
    let new = fs::read(&header_a)?;

    // (a) Crash after slot A: B still has the old generation. The new password works and heals B.
    fs::write(&header_b, &old)?;
    let vault = fixture.open()?;
    vault.unlock(&pw(OTHER_PASSWORD))?;
    assert_eq!(fs::read(&header_b)?, new);
    vault.lock(LockPolicy::SyncThenWipe)?;
    drop(vault);

    // (b) Slot A torn mid-write: the old password still works.
    fs::write(&header_a, &new[..70])?;
    fs::write(&header_b, &old)?;
    let vault = fixture.open()?;
    assert_eq!(vault.status().kdf, Some(HeaderSlot::decode(&old)?.kdf));
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(fs::read(&header_a)?, old, "torn slot A rewritten from B");
    Ok(())
}

#[test]
fn plain_file_dropped_in_vault_is_ingested_and_original_removed_on_unlock() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "notes.txt", b"already inside")?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    drop(vault);

    // Dropped with Explorer while locked: a file, a name clash and a folder.
    fs::write(fixture.root().join("notes.txt"), b"plain notes")?;
    fs::write(fixture.root().join("plan.md"), b"# plain plan")?;
    fs::create_dir_all(fixture.root().join("Photos").join("2025"))?;
    fs::write(
        fixture.root().join("Photos").join("2025").join("a.jpg"),
        b"jpeg bytes",
    )?;

    let vault = fixture.open()?;
    let status = vault.status();
    assert_eq!(status.state, VaultState::Locked);
    assert_eq!(status.foreign_items, 3);

    vault.unlock(&pw(PASSWORD))?;
    for name in ["notes.txt", "plan.md", "Photos"] {
        assert!(
            !fixture.root().join(name).exists(),
            "{name} original removed"
        );
    }
    assert_eq!(vault.status().foreign_items, 0);
    assert_eq!(
        common::names(&vault, "")?,
        vec![
            "Photos".to_owned(),
            "notes (2).txt".to_owned(),
            "notes.txt".to_owned(),
            "plan.md".to_owned()
        ]
    );
    assert_eq!(
        fixture.export_bytes(&vault, "notes.txt")?,
        b"already inside"
    );
    assert_eq!(
        fixture.export_bytes(&vault, "notes (2).txt")?,
        b"plain notes"
    );
    assert_eq!(
        fixture.export_bytes(&vault, "Photos/2025/a.jpg")?,
        b"jpeg bytes"
    );

    // Nothing plain is left anywhere in the vault folder.
    for (path, bytes) in common::snapshot(fixture.root())? {
        for marker in [&b"plain notes"[..], b"# plain plan", b"jpeg bytes"] {
            assert!(
                !common::contains_bytes(&bytes, marker),
                "{} holds plaintext",
                path.display()
            );
        }
    }
    Ok(())
}

//! The session folder: stale cleanup, sync and lock (C5, S1 to S7, N5).

// cspell:ignore crdownload

mod common;

use std::fs;
use std::time::{Duration, SystemTime};

use common::{Fixture, PASSWORD, TestResult, count_files, pw};
use genslate_vault::{LockPolicy, Vault, VaultError, VaultPath, VaultState};

#[test]
fn stale_session_folder_wiped_on_open() -> TestResult {
    let fixture = Fixture::new()?;
    let stale = fixture.session_root().join("deadbeef");
    fs::create_dir_all(stale.join("0001"))?;
    fs::create_dir_all(stale.join("0002").join("nested").join("deeper"))?;
    fs::create_dir_all(stale.join("0003"))?; // empty folder
    fs::write(
        stale.join("0001").join("tax return.pdf"),
        b"PLAINTEXT-MARKER-1",
    )?;
    fs::write(
        stale
            .join("0002")
            .join("nested")
            .join("deeper")
            .join("x.txt"),
        b"PLAINTEXT-MARKER-2",
    )?;
    let long_name = format!("{}.txt", "n".repeat(200));
    fs::write(stale.join("0002").join(&long_name), b"PLAINTEXT-MARKER-3")?;
    let read_only = stale.join("0001").join("read-only.txt");
    fs::write(&read_only, b"PLAINTEXT-MARKER-4")?;
    let mut perms = fs::metadata(&read_only)?.permissions();
    perms.set_readonly(true);
    fs::set_permissions(&read_only, perms)?;
    // Not a session folder: left alone and reported.
    let loose = fixture.session_root().join("loose.txt");
    fs::write(&loose, b"NOT-OURS")?;

    let (vault, report) = Vault::open(fixture.config.clone())?;
    assert_eq!(report.stale_files_wiped, 4);
    assert_eq!(report.stale_files_left, vec![loose.clone()]);
    assert!(
        fixture.session_root().is_dir(),
        "the session root itself stays"
    );
    assert!(!stale.exists(), "the stale session folder is gone");
    assert_eq!(fs::read(&loose)?, b"NOT-OURS");
    assert_eq!(vault.status().state, VaultState::Uninitialized);
    Ok(())
}

#[test]
fn session_wipe_stays_inside_session_folders() -> TestResult {
    let fixture = Fixture::new()?;
    let outside_dir = fixture.scratch()?.join("outside");
    fs::create_dir_all(&outside_dir)?;
    let outside_file = outside_dir.join("keep.txt");
    fs::write(&outside_file, b"KEEP-ME")?;
    let shared = fixture.scratch()?.join("shared.txt");
    fs::write(&shared, b"SHARED-INODE")?;

    let stale = fixture.session_root().join("0badc0de");
    fs::create_dir_all(stale.join("0001"))?;
    fs::write(stale.join("0001").join("doc.txt"), b"PLAINTEXT")?;
    fs::hard_link(&shared, stale.join("0001").join("hard.txt"))?;
    common::symlink_file(&outside_file, &stale.join("0001").join("link.txt"))?;
    common::symlink_dir(&outside_dir, &stale.join("0002"))?;
    // An entry named like a session that is itself a link to an outside folder.
    let session_link = fixture.session_root().join("feedf00d");
    let root_link = common::symlink_dir(&outside_dir, &session_link)?;
    // Not a session id: never touched.
    let notes = fixture.session_root().join("notes");
    fs::create_dir_all(&notes)?;
    fs::write(notes.join("todo.txt"), b"NOT-A-SESSION")?;

    let (_vault, report) = Vault::open(fixture.config.clone())?;
    assert!(!stale.exists(), "the stale session folder is gone");
    assert_eq!(fs::read(&outside_file)?, b"KEEP-ME");
    assert_eq!(fs::read_dir(&outside_dir)?.count(), 1);
    if cfg!(unix) {
        // Windows reports no link count to safe Rust; see the task report.
        assert_eq!(
            fs::read(&shared)?,
            b"SHARED-INODE",
            "a hard-linked file is unlinked, not zeroed"
        );
    }
    if root_link {
        assert!(
            fs::symlink_metadata(&session_link).is_err(),
            "the link itself is removed"
        );
    }
    assert_eq!(fs::read(notes.join("todo.txt"))?, b"NOT-A-SESSION");
    assert_eq!(report.stale_files_left, vec![notes]);
    Ok(())
}

#[test]
fn unreadable_edit_blocks_sync_then_wipe_and_force_reports_it() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"original")?;
    let session_file = vault.open_in_session(&VaultPath::parse("a.txt")?, None)?;
    // Something that cannot be read back as the file, even by root (research S8).
    fs::remove_file(&session_file)?;
    fs::create_dir(&session_file)?;
    fs::write(session_file.join("inner.txt"), b"x")?;

    match vault.lock(LockPolicy::SyncThenWipe) {
        Err(VaultError::UnsyncedEdits(paths)) => {
            assert_eq!(paths, vec![VaultPath::parse("a.txt")?]);
        }
        other => return Err(format!("expected UnsyncedEdits, got {other:?}").into()),
    }
    assert_eq!(vault.status().state, VaultState::Unlocked);
    assert!(session_file.exists(), "nothing is wiped while unlocked");

    let report = vault.lock(LockPolicy::Force)?;
    assert_eq!(report.unsynced, vec![VaultPath::parse("a.txt")?]);
    assert_eq!(count_files(fixture.session_root())?, 0);
    assert_eq!(vault.status().state, VaultState::Locked);
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(fixture.export_bytes(&vault, "a.txt")?, b"original");
    Ok(())
}

#[test]
fn lock_syncs_edits_then_wipes_session() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    vault.create_dir(&VaultPath::parse("Docs")?)?;
    fixture.import_bytes(&vault, "Docs", "doc.txt", b"original text")?;
    let blobs_before = fixture.blobs()?;

    let session_file = vault.open_in_session(&VaultPath::parse("Docs/doc.txt")?, None)?;
    assert!(session_file.starts_with(fixture.session_root()));
    assert_eq!(
        session_file.file_name().and_then(|n| n.to_str()),
        Some("doc.txt")
    );
    assert_eq!(fs::read(&session_file)?, b"original text");
    assert_eq!(vault.status().session_files, 1);
    // Opening twice returns the same path.
    assert_eq!(
        vault.open_in_session(&VaultPath::parse("Docs/doc.txt")?, None)?,
        session_file
    );

    // Edit it, and let an editor drop a "Save As" sibling plus its lock and temp files.
    fs::write(&session_file, b"edited text, longer than before")?;
    let folder = session_file.parent().ok_or("no parent")?;
    fs::write(folder.join("copy.txt"), b"saved as")?;
    fs::write(folder.join("~$doc.txt"), b"office lock")?;
    fs::write(folder.join(".~lock.doc.txt#"), b"libreoffice lock")?;
    fs::write(folder.join("x.tmp"), b"temp")?;
    fs::write(folder.join("dl.crdownload"), b"partial download")?;

    let report = vault.lock(LockPolicy::SyncThenWipe)?;
    assert!(
        report.synced.contains(&VaultPath::parse("Docs/doc.txt")?),
        "{report:?}"
    );
    assert!(
        report.synced.contains(&VaultPath::parse("Docs/copy.txt")?),
        "{report:?}"
    );
    common::assert_empty(&report.unsynced);
    common::assert_empty(&report.undeletable);
    assert!(report.wiped_files >= 6);
    assert_eq!(
        count_files(fixture.session_root())?,
        0,
        "session folder wiped"
    );
    assert_eq!(vault.status().state, VaultState::Locked);
    assert_eq!(vault.status().session_files, 0);
    assert!(matches!(
        vault.list(&VaultPath::root()),
        Err(VaultError::Locked)
    ));

    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(
        fixture.export_bytes(&vault, "Docs/doc.txt")?,
        b"edited text, longer than before"
    );
    assert_eq!(fixture.export_bytes(&vault, "Docs/copy.txt")?, b"saved as");
    assert_eq!(
        common::names(&vault, "Docs")?,
        vec!["copy.txt".to_owned(), "doc.txt".to_owned()]
    );
    for old in &blobs_before {
        assert!(!old.exists(), "the superseded blob is deleted");
    }
    Ok(())
}

#[test]
fn sync_session_writes_only_real_changes() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"same length A")?;
    let session_file = vault.open_in_session(&VaultPath::parse("a.txt")?, None)?;
    let past = SystemTime::now() - Duration::from_secs(120);

    // Touch only: mtime changes, content does not. No new blob.
    let blobs = fixture.blobs()?;
    fs::File::options()
        .write(true)
        .open(&session_file)?
        .set_modified(past)?;
    let report = vault.sync_session()?;
    common::assert_empty(&report.updated);
    assert_eq!(fixture.blobs()?, blobs);

    // Same length, different content, mtime set back into the past: detected by the hash.
    fs::write(&session_file, b"same length B")?;
    fs::File::options()
        .write(true)
        .open(&session_file)?
        .set_modified(past - Duration::from_secs(60))?;
    let report = vault.sync_session()?;
    assert_eq!(report.updated, vec![VaultPath::parse("a.txt")?]);
    assert_ne!(fixture.blobs()?, blobs);
    assert_eq!(fixture.export_bytes(&vault, "a.txt")?, b"same length B");

    // An atomic-save editor replaces the file by rename.
    let temp = session_file.with_file_name("a.txt.new");
    fs::write(&temp, b"replaced by rename")?;
    fs::rename(&temp, &session_file)?;
    fs::File::options()
        .write(true)
        .open(&session_file)?
        .set_modified(past)?;
    let report = vault.sync_session()?;
    assert_eq!(report.updated, vec![VaultPath::parse("a.txt")?]);
    assert_eq!(
        fixture.export_bytes(&vault, "a.txt")?,
        b"replaced by rename"
    );

    // Deleting the session copy forgets it and keeps the vault entry.
    fs::remove_file(&session_file)?;
    let report = vault.sync_session()?;
    assert!(
        report.updated.is_empty() && report.failed.is_empty(),
        "{report:?}"
    );
    assert_eq!(vault.status().session_files, 0);
    assert_eq!(
        fixture.export_bytes(&vault, "a.txt")?,
        b"replaced by rename"
    );
    Ok(())
}

#[test]
fn session_path_fits_windows_max_path_and_maps_back() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.config.session_root = fixture.dir.path().join("x".repeat(60)).join("y".repeat(60));
    let vault = fixture.create()?;
    let long = format!("{}.txt", "L".repeat(251));
    assert_eq!(long.encode_utf16().count(), 255);
    fixture.import_bytes(&vault, "", "short.txt", b"long name content")?;
    vault.rename(&VaultPath::parse("short.txt")?, &VaultPath::parse(&long)?)?;

    let session_file = vault.open_in_session(&VaultPath::parse(&long)?, None)?;
    let text = session_file.to_str().ok_or("not unicode")?;
    assert!(
        text.encode_utf16().count() <= 259,
        "{} units",
        text.encode_utf16().count()
    );
    assert!(
        session_file.extension().is_some_and(|e| e == "txt"),
        "extension kept: {text}"
    );
    fs::write(&session_file, b"edited through a shortened name")?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(
        fixture.export_bytes(&vault, &long)?,
        b"edited through a shortened name"
    );
    Ok(())
}

#[test]
fn delete_and_rename_follow_open_session_files() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"a")?;
    fixture.import_bytes(&vault, "", "b.txt", b"b")?;
    let a = vault.open_in_session(&VaultPath::parse("a.txt")?, None)?;
    let b = vault.open_in_session(&VaultPath::parse("b.txt")?, None)?;

    vault.rename(
        &VaultPath::parse("a.txt")?,
        &VaultPath::parse("renamed.txt")?,
    )?;
    fs::write(&a, b"edited after rename")?;
    vault.delete(&VaultPath::parse("b.txt")?)?;
    assert!(!b.exists(), "a deleted entry's plaintext copy is wiped");

    let report = vault.lock(LockPolicy::SyncThenWipe)?;
    assert_eq!(report.synced, vec![VaultPath::parse("renamed.txt")?]);
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(
        fixture.export_bytes(&vault, "renamed.txt")?,
        b"edited after rename"
    );
    assert_eq!(common::names(&vault, "")?, vec!["renamed.txt".to_owned()]);
    Ok(())
}

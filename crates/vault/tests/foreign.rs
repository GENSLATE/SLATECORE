//! Plain items dropped into the vault folder (spec A3 D4): only what was encrypted and verified
//! is wiped, and a folder that cannot be imported is left whole.

mod common;

use std::fs;
use std::io::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};

use common::{Fixture, PASSWORD, TestResult, pseudo_random, pw};
use genslate_vault::{LockPolicy, VaultPath};

#[test]
fn foreign_ingest_wipes_only_files_it_encrypted() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    let photos = fixture.root().join("Photos");
    fs::create_dir_all(photos.join("sub"))?;
    let mut a = pseudo_random(300_000, 1);
    let mut b = pseudo_random(300_000, 2);
    fs::write(photos.join("a.bin"), &a)?;
    fs::write(photos.join("b.bin"), &b)?;
    fs::write(photos.join("sub").join("c.txt"), b"unchanged")?;
    let outside = fixture.scratch()?.join("outside.txt");
    fs::write(&outside, b"OUTSIDE-THE-VAULT")?;
    let linked = common::symlink_file(&outside, &photos.join("link.txt"))?;

    let fired = AtomicBool::new(false);
    let (fired_ref, photos_ref) = (&fired, &photos);
    let progress = move |done: u64, _total: u64| {
        if done > 0 && !fired_ref.swap(true, Ordering::SeqCst) {
            // While the first file is encrypted: a file appears and two files grow.
            let _ = fs::write(photos_ref.join("late.txt"), b"arrived during the import");
            for name in ["a.bin", "b.bin"] {
                if let Ok(mut file) = fs::OpenOptions::new()
                    .append(true)
                    .open(photos_ref.join(name))
                {
                    let _ = file.write_all(b"-grown");
                }
            }
        }
    };
    vault.import_foreign(Some(&progress))?;
    assert!(fired.load(Ordering::SeqCst), "the import reported progress");
    a.extend_from_slice(b"-grown");
    b.extend_from_slice(b"-grown");

    // Never encrypted, so never wiped.
    assert_eq!(
        fs::read(photos.join("late.txt"))?,
        b"arrived during the import"
    );
    // A file that changed after it was listed keeps its plaintext original.
    assert_eq!(fs::read(photos.join("a.bin"))?, a);
    assert_eq!(fs::read(photos.join("b.bin"))?, b);
    // The unchanged file is in the vault and its original is gone.
    assert_eq!(
        fixture.export_bytes(&vault, "Photos/sub/c.txt")?,
        b"unchanged"
    );
    assert!(!photos.join("sub").join("c.txt").exists());
    assert!(!photos.join("sub").exists(), "an emptied folder is removed");
    if linked {
        let meta = fs::symlink_metadata(photos.join("link.txt"))?;
        assert!(
            meta.file_type().is_symlink(),
            "a link is never imported or removed"
        );
    }
    assert_eq!(fs::read(&outside)?, b"OUTSIDE-THE-VAULT");
    // What is left is still foreign.
    assert_eq!(vault.status().foreign_items, 1);
    Ok(())
}

#[test]
fn a_foreign_folder_that_cannot_be_imported_is_left_whole_and_not_duplicated() -> TestResult {
    let fixture = Fixture::new()?;
    let photos = fixture.root().join("Photos");
    let mut deep = photos.clone();
    for level in 1..=16 {
        deep = deep.join(format!("l{level:02}")); // 17 levels with Photos: one too many
    }
    fs::create_dir_all(&deep)?;
    fs::write(deep.join("deep.txt"), b"too deep")?;
    fs::write(photos.join("ok.txt"), b"fine on its own")?;

    let vault = fixture.create()?; // create ingests foreign items
    for round in 0..3 {
        if round > 0 {
            vault.lock(LockPolicy::SyncThenWipe)?;
            vault.unlock(&pw(PASSWORD))?;
        }
        common::assert_empty(&common::names(&vault, "")?);
        common::assert_empty(&fixture.blobs()?);
        assert_eq!(fs::read(photos.join("ok.txt"))?, b"fine on its own");
        assert_eq!(fs::read(deep.join("deep.txt"))?, b"too deep");
        assert_eq!(vault.status().foreign_items, 1, "round {round}");
    }
    Ok(())
}

#[test]
fn an_explicit_folder_import_is_one_commit() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    let src = fixture.scratch()?.join("Tree");
    fs::create_dir_all(src.join("x").join("y"))?;
    fs::write(src.join("one.txt"), b"1")?;
    fs::write(src.join("x").join("two.txt"), b"2")?;
    fs::write(src.join("x").join("y").join("three.txt"), b"3")?;
    let before = common::index_generation(&fixture.root().join("index.b.gvi"))?;
    vault.import(
        &src,
        &VaultPath::root(),
        genslate_vault::Conflict::Fail,
        None,
    )?;
    let after = [
        common::index_generation(&fixture.root().join("index.a.gvi"))?,
        common::index_generation(&fixture.root().join("index.b.gvi"))?,
    ];
    assert_eq!(after.iter().max().copied(), Some(before + 1), "{after:?}");
    assert_eq!(fixture.export_bytes(&vault, "Tree/x/y/three.txt")?, b"3");
    assert_eq!(
        common::names(&vault, "Tree")?,
        vec!["one.txt".to_owned(), "x".to_owned()]
    );
    assert!(
        src.join("one.txt").exists(),
        "an explicit import leaves the source"
    );
    Ok(())
}

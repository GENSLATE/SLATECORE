//! Tampering, truncation, swapping and rollback (T1 to T8, P6, P8).

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{Fixture, OTHER_PASSWORD, PASSWORD, TestResult, pseudo_random, pw};
use genslate_vault::format::header::HeaderSlot;
use genslate_vault::format::index::IndexSlotHeader;
use genslate_vault::{LockPolicy, Problem, Vault, VaultError, VaultPath};

const C: usize = 65_536;

/// Exports `path` and asserts it fails with neither the destination nor a `.gvpart` left.
fn assert_export_fails(
    fixture: &Fixture,
    vault: &Vault,
    path: &str,
) -> Result<VaultError, Box<dyn std::error::Error>> {
    let dest = fixture.scratch()?.join(format!("out-{}", common::unique()));
    let result = vault.export(&VaultPath::parse(path)?, &dest, None);
    assert!(!dest.exists(), "no destination file after a failed export");
    let part = PathBuf::from(format!("{}.gvpart", dest.display()));
    assert!(!part.exists(), "no partial file after a failed export");
    match result {
        Ok(()) => Err("export unexpectedly succeeded".into()),
        Err(e) => Ok(e),
    }
}

fn only_blob(fixture: &Fixture) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let blobs = fixture.blobs()?;
    assert_eq!(blobs.len(), 1);
    Ok(blobs[0].clone())
}

fn flip(path: &Path, offset: usize) -> std::io::Result<()> {
    let mut bytes = fs::read(path)?;
    bytes[offset] ^= 0x01;
    fs::write(path, bytes)
}

#[test]
fn tampered_ciphertext_rejected() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    let plain = pseudo_random(2 * C + 100, 1);
    fixture.import_bytes(&vault, "", "data.bin", &plain)?;
    let blob = only_blob(&fixture)?;
    let original = fs::read(&blob)?;
    let len = original.len();

    // magic, version, flags, chunk_size, blob_id, stream_nonce, reserved,
    // first/middle/last chunk ciphertext, a tag byte.
    let offsets = [
        0,
        8,
        10,
        12,
        16,
        32,
        51,
        64 + 10,
        64 + C + 16 + 10,
        len - 30,
        len - 1,
    ];
    for offset in offsets {
        fs::write(&blob, &original)?;
        flip(&blob, offset)?;
        let err = assert_export_fails(&fixture, &vault, "data.bin")?;
        assert!(
            matches!(
                err,
                VaultError::Tampered(_) | VaultError::UnsupportedVersion(_)
            ),
            "offset {offset}: {err:?}"
        );
        let report = vault.verify(None)?;
        assert_eq!(report.problems.len(), 1, "offset {offset}");
        // open_in_session never releases plaintext either.
        assert!(
            vault
                .open_in_session(&VaultPath::parse("data.bin")?, None)
                .is_err()
        );
        assert_eq!(
            common::count_files(fixture.session_root())?,
            0,
            "offset {offset}"
        );
    }
    fs::write(&blob, &original)?;
    assert_eq!(fixture.export_bytes(&vault, "data.bin")?, plain);
    Ok(())
}

#[test]
fn truncated_blob_rejected() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "data.bin", &pseudo_random(2 * C + 100, 2))?;
    fixture.import_bytes(&vault, "", "empty.bin", b"")?;
    let blobs = fixture.blobs()?;
    let (big, empty) = if fs::metadata(&blobs[0])?.len() == 80 {
        (blobs[1].clone(), blobs[0].clone())
    } else {
        (blobs[0].clone(), blobs[1].clone())
    };

    let original = fs::read(&big)?;
    let len = original.len();
    let boundary1 = 64 + (C + 16);
    let boundary2 = 64 + 2 * (C + 16);
    for cut in [
        0,
        63,
        64,
        79,
        boundary1,
        boundary2,
        boundary1 + 100,
        len - 1,
    ] {
        fs::write(&big, &original[..cut])?;
        let err = assert_export_fails(&fixture, &vault, "data.bin")?;
        assert!(
            matches!(err, VaultError::Tampered(_)),
            "cut at {cut}: {err:?}"
        );
    }
    // Extended: an appended whole chunk, appended garbage, one extra byte.
    let mut appended_chunk = original.clone();
    appended_chunk.extend_from_slice(&original[64..64 + C + 16]);
    for extended in [
        appended_chunk,
        [original.clone(), vec![0u8; 40]].concat(),
        [original.clone(), vec![7u8]].concat(),
    ] {
        fs::write(&big, &extended)?;
        let err = assert_export_fails(&fixture, &vault, "data.bin")?;
        assert!(matches!(err, VaultError::Tampered(_)), "{err:?}");
    }
    fs::write(&big, &original)?;

    let empty_bytes = fs::read(&empty)?;
    for cut in [64, 79] {
        fs::write(&empty, &empty_bytes[..cut])?;
        assert_export_fails(&fixture, &vault, "empty.bin")?;
    }
    fs::write(&empty, &empty_bytes)?;
    let report = vault.verify(None)?;
    common::assert_empty(&report.problems);
    assert_eq!(fixture.export_bytes(&vault, "empty.bin")?, b"");
    Ok(())
}

#[test]
fn swapped_blobs_rejected() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.bin", &pseudo_random(1000, 3))?;
    let blob_a = only_blob(&fixture)?;
    fixture.import_bytes(&vault, "", "b.bin", &pseudo_random(1000, 4))?;
    let blob_b = fixture
        .blobs()?
        .into_iter()
        .find(|p| *p != blob_a)
        .ok_or("no second blob")?;
    let bytes_a = fs::read(&blob_a)?;
    let bytes_b = fs::read(&blob_b)?;

    // 1. Exchange the files under each other's names: the header id no longer matches.
    fs::write(&blob_a, &bytes_b)?;
    fs::write(&blob_b, &bytes_a)?;
    for name in ["a.bin", "b.bin"] {
        assert!(matches!(
            assert_export_fails(&fixture, &vault, name)?,
            VaultError::Tampered(_)
        ));
    }

    // 2. Same, but also patch the header id to match the file name: the id is in the AD.
    let mut patched = bytes_b.clone();
    patched[16..32].copy_from_slice(&bytes_a[16..32]);
    fs::write(&blob_a, &patched)?;
    fs::write(&blob_b, &bytes_b)?;
    assert!(matches!(
        assert_export_fails(&fixture, &vault, "a.bin")?,
        VaultError::Tampered(_)
    ));
    assert_eq!(
        fixture.export_bytes(&vault, "b.bin")?,
        pseudo_random(1000, 4)
    );

    // 3. Keep a's header, use b's body.
    let mut spliced = bytes_a[..64].to_vec();
    spliced.extend_from_slice(&bytes_b[64..]);
    fs::write(&blob_a, &spliced)?;
    assert!(matches!(
        assert_export_fails(&fixture, &vault, "a.bin")?,
        VaultError::Tampered(_)
    ));

    // 4. Version is bound too: a different format version is refused.
    let mut versioned = bytes_a.clone();
    versioned[8] = 2;
    fs::write(&blob_a, &versioned)?;
    assert!(matches!(
        assert_export_fails(&fixture, &vault, "a.bin")?,
        VaultError::UnsupportedVersion(2)
    ));

    // 5. A missing blob is reported as such.
    fs::remove_file(&blob_a)?;
    assert!(matches!(
        assert_export_fails(&fixture, &vault, "a.bin")?,
        VaultError::BlobMissing
    ));
    let report = vault.verify(None)?;
    assert_eq!(report.problems.len(), 1);
    assert_eq!(report.problems[0].1, Problem::BlobMissing);

    fs::write(&blob_a, &bytes_a)?;
    assert_eq!(
        fixture.export_bytes(&vault, "a.bin")?,
        pseudo_random(1000, 3)
    );
    Ok(())
}

fn index_generation(path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(IndexSlotHeader::decode(&fs::read(path)?)?.generation)
}

#[test]
fn rollback_to_older_generation_rejected_when_newer_slot_valid() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    let index_a = fixture.root().join("index.a.gvi");
    let index_b = fixture.root().join("index.b.gvi");
    let header_b = fixture.root().join("vault.b.gvh");
    let old_header = fs::read(&header_b)?; // generation 1, PASSWORD

    fixture.import_bytes(&vault, "", "a.txt", b"first")?; // index generation 2 -> slot A
    let old_index_b = fs::read(&index_b)?; // generation 1
    fixture.import_bytes(&vault, "", "b.txt", b"second")?; // generation 3 -> slot B
    assert_eq!(index_generation(&index_a)?, 2);
    assert_eq!(index_generation(&index_b)?, 3);
    vault.change_password(&pw(PASSWORD), &pw(OTHER_PASSWORD))?;
    vault.lock(LockPolicy::SyncThenWipe)?;

    // Index: an older generation in the other slot is ignored...
    fs::write(&index_a, &old_index_b)?;
    // ...and relabelling it with a higher generation breaks its authentication.
    let mut relabelled = old_index_b.clone();
    relabelled[12..20].copy_from_slice(&99u64.to_le_bytes());
    fs::write(&index_a, &relabelled)?;

    // Header: the stale slot with the old password must not be used as a fallback.
    fs::write(&header_b, &old_header)?;
    drop(vault);
    let vault = fixture.open()?;
    assert!(matches!(
        vault.unlock(&pw(PASSWORD)),
        Err(VaultError::WrongPassword)
    ));
    vault.unlock(&pw(OTHER_PASSWORD))?;
    assert_eq!(
        common::names(&vault, "")?,
        vec!["a.txt".to_owned(), "b.txt".to_owned()]
    );
    assert_eq!(fixture.export_bytes(&vault, "b.txt")?, b"second");

    // The successful unlock healed the stale header slot and the broken index slot.
    assert_eq!(
        fs::read(&header_b)?,
        fs::read(fixture.root().join("vault.a.gvh"))?
    );
    assert_eq!(index_generation(&index_a)?, 3);
    Ok(())
}

#[test]
fn header_checksum_failure_falls_back_and_both_bad_is_damaged() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    drop(vault);
    let header_a = fixture.root().join("vault.a.gvh");
    let header_b = fixture.root().join("vault.b.gvh");
    let good = fs::read(&header_a)?;

    flip(&header_a, 100)?; // checksum no longer matches: slot A is skipped
    let vault = fixture.open()?;
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(fs::read(&header_a)?, good, "slot A healed from slot B");
    vault.lock(LockPolicy::SyncThenWipe)?;
    drop(vault);

    flip(&header_a, 100)?;
    flip(&header_b, 100)?;
    let vault = fixture.open()?;
    assert!(matches!(
        vault.unlock(&pw(PASSWORD)),
        Err(VaultError::HeaderDamaged)
    ));
    assert_eq!(
        vault.status().failed_attempts,
        0,
        "a damaged header is not a wrong password"
    );

    // Valid checksum but a tampered wrapped key: the winner fails, no fallback.
    fs::write(&header_a, &good)?;
    drop(vault);
    let mut slot = HeaderSlot::decode(&good)?;
    slot.generation = 2;
    slot.wrapped_vk[0] ^= 1;
    fs::write(&header_b, slot.encode())?;
    let vault = fixture.open()?;
    assert!(matches!(
        vault.unlock(&pw(PASSWORD)),
        Err(VaultError::WrongPassword)
    ));
    Ok(())
}

#[test]
fn damaged_index_slots() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"first")?; // generation 2 in slot A
    vault.lock(LockPolicy::SyncThenWipe)?;
    drop(vault);
    let index_a = fixture.root().join("index.a.gvi");
    let index_b = fixture.root().join("index.b.gvi");
    let good_a = fs::read(&index_a)?;
    let good_b = fs::read(&index_b)?;

    // Both damaged: unlock refuses and garbage collection deletes nothing.
    flip(&index_a, 20)?;
    flip(&index_b, 30)?;
    let blobs_before = fixture.blobs()?;
    assert_eq!(blobs_before.len(), 1);
    let vault = fixture.open()?;
    assert!(matches!(
        vault.unlock(&pw(PASSWORD)),
        Err(VaultError::Tampered("index"))
    ));
    assert_eq!(vault.status().state, genslate_vault::VaultState::Locked);
    assert_eq!(fixture.blobs()?, blobs_before);
    drop(vault);

    // Newest slot's tag damaged: falls back to generation 1 (documented limit, T9).
    fs::write(&index_a, &good_a)?;
    fs::write(&index_b, &good_b)?;
    flip(&index_a, good_a.len() - 1)?;
    let vault = fixture.open()?;
    vault.unlock(&pw(PASSWORD))?;
    common::assert_empty(&common::names(&vault, "")?);
    Ok(())
}

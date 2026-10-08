//! Round trips, password handling and API state rules (F6, F7, P1 to P5, P7, C6, H1).

mod common;

use std::fs;

use common::{Fixture, OTHER_PASSWORD, PASSWORD, TestResult, pseudo_random, pw, snapshot};
use genslate_vault::format::blob::encrypted_len;
use genslate_vault::format::header::HeaderSlot;
use genslate_vault::{
    Conflict, EntryKind, ExposeSecret, KdfParams, LockPolicy, Vault, VaultConfig, VaultError,
    VaultPath, VaultState, kdf,
};

const C: usize = 65_536;

#[test]
fn roundtrip_small_empty_and_large_stream() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    let lengths = [
        0,
        1,
        C - 1,
        C,
        C + 1,
        2 * C,
        3 * C + 17,
        8 * 1024 * 1024 + 5,
    ];
    for (i, len) in lengths.iter().enumerate() {
        let bytes = pseudo_random(*len, i as u64 + 1);
        fixture.import_bytes(&vault, "", &format!("file-{i}.bin"), &bytes)?;
    }

    // Each blob has exactly the documented length.
    let mut blob_lengths: Vec<u64> = fixture
        .blobs()?
        .iter()
        .map(|p| fs::metadata(p).map(|m| m.len()))
        .collect::<Result<_, _>>()?;
    let mut expected: Vec<u64> = lengths
        .iter()
        .map(|l| encrypted_len(*l as u64, 65_536))
        .collect();
    blob_lengths.sort_unstable();
    expected.sort_unstable();
    assert_eq!(blob_lengths, expected);
    assert_eq!(
        encrypted_len(0, 65_536),
        80,
        "empty file is an 80-byte blob"
    );

    for (i, len) in lengths.iter().enumerate() {
        let got = fixture.export_bytes(&vault, &format!("file-{i}.bin"))?;
        assert_eq!(got, pseudo_random(*len, i as u64 + 1), "length {len}");
    }

    // Survives lock and unlock (the index is re-read from disk).
    vault.lock(LockPolicy::SyncThenWipe)?;
    drop(vault);
    let vault = fixture.open()?;
    vault.unlock(&pw(PASSWORD))?;
    let listed = vault.list(&VaultPath::root())?;
    assert_eq!(listed.len(), lengths.len());
    for entry in &listed {
        assert_eq!(entry.kind, EntryKind::File);
    }
    let big = fixture.export_bytes(&vault, "file-7.bin")?;
    assert_eq!(big, pseudo_random(lengths[7], 8));
    Ok(())
}

#[test]
fn wrong_password_leaves_locked_and_changes_no_file() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"alpha")?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    let before = snapshot(fixture.root())?;

    let result = vault.unlock(&pw(OTHER_PASSWORD));
    assert!(
        matches!(result, Err(VaultError::WrongPassword)),
        "{result:?}"
    );
    let status = vault.status();
    assert_eq!(status.state, VaultState::Locked);
    assert_eq!(status.failed_attempts, 1);
    assert!(matches!(
        vault.list(&VaultPath::root()),
        Err(VaultError::Locked)
    ));

    let mut after = snapshot(fixture.root())?;
    let mut before = before;
    before.remove(std::path::Path::new("vault.guard"));
    after.remove(std::path::Path::new("vault.guard"));
    assert_eq!(
        before, after,
        "only vault.guard may change on a wrong password"
    );

    // P2: the right password still works and resets the counter.
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(vault.status().failed_attempts, 0);
    assert_eq!(fixture.export_bytes(&vault, "a.txt")?, b"alpha");
    Ok(())
}

#[test]
fn password_change_rewraps_only_header() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"alpha")?;
    let header_a = fixture.root().join("vault.a.gvh");
    let header_b = fixture.root().join("vault.b.gvh");
    let old = HeaderSlot::decode(&fs::read(&header_a)?)?;
    let mut data_before = snapshot(fixture.root())?;
    for name in ["vault.a.gvh", "vault.b.gvh", "vault.guard", "vault.lock"] {
        data_before.remove(std::path::Path::new(name));
    }

    vault.change_password(&pw(PASSWORD), &pw(OTHER_PASSWORD))?;

    let slot_a = fs::read(&header_a)?;
    let slot_b = fs::read(&header_b)?;
    assert_eq!(slot_a, slot_b, "both header slots identical");
    let new = HeaderSlot::decode(&slot_a)?;
    assert_eq!(new.generation, old.generation + 1);
    assert_eq!(new.vault_id, old.vault_id);
    assert_ne!(new.salt, old.salt);
    assert_ne!(new.wrap_nonce, old.wrap_nonce);

    let mut data_after = snapshot(fixture.root())?;
    for name in ["vault.a.gvh", "vault.b.gvh", "vault.guard", "vault.lock"] {
        data_after.remove(std::path::Path::new(name));
    }
    assert_eq!(
        data_before, data_after,
        "index and blobs are byte-identical"
    );

    vault.lock(LockPolicy::SyncThenWipe)?;
    assert!(matches!(
        vault.unlock(&pw(PASSWORD)),
        Err(VaultError::WrongPassword)
    ));
    vault.unlock(&pw(OTHER_PASSWORD))?;
    assert_eq!(fixture.export_bytes(&vault, "a.txt")?, b"alpha");

    // A wrong current password is refused and changes nothing.
    let result = vault.change_password(&pw(PASSWORD), &pw("yet another passphrase"));
    assert!(matches!(result, Err(VaultError::WrongPassword)));
    assert_eq!(fs::read(&header_a)?, slot_a);
    Ok(())
}

#[test]
fn password_policy_is_enforced_and_creates_no_files() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.open()?;
    for bad in ["", "short", "1234567"] {
        assert!(
            matches!(vault.create(&pw(bad)), Err(VaultError::PasswordRejected(_))),
            "{bad:?}"
        );
    }
    let too_long = "x".repeat(1025);
    assert!(matches!(
        vault.create(&pw(&too_long)),
        Err(VaultError::PasswordRejected(_))
    ));
    assert!(!fixture.root().join("vault.a.gvh").exists());
    assert!(!fixture.root().join("index.b.gvi").exists());
    assert_eq!(vault.status().state, VaultState::Uninitialized);

    // Exactly 8 and exactly 1024 bytes are fine.
    vault.create(&pw(&"y".repeat(1024)))?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    vault.unlock(&pw(&"y".repeat(1024)))?;
    Ok(())
}

#[test]
fn nfkc_equivalent_passwords_unlock() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.open()?;
    vault.create(&pw("caf\u{e9} au lait"))?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    vault.unlock(&pw("cafe\u{301} au lait"))?;
    Ok(())
}

#[test]
fn state_rules_for_every_method() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.open()?;
    let root = VaultPath::root();
    assert_eq!(vault.status().state, VaultState::Uninitialized);
    assert!(matches!(
        vault.unlock(&pw(PASSWORD)),
        Err(VaultError::Uninitialized)
    ));

    vault.create(&pw(PASSWORD))?;
    assert_eq!(vault.status().state, VaultState::Unlocked);
    assert!(matches!(
        vault.create(&pw(PASSWORD)),
        Err(VaultError::AlreadyExists)
    ));
    assert!(matches!(
        vault.unlock(&pw(PASSWORD)),
        Err(VaultError::AlreadyUnlocked)
    ));
    vault.lock(LockPolicy::SyncThenWipe)?;

    let scratch = fixture.scratch()?;
    let src = scratch.join("x.txt");
    fs::write(&src, b"x")?;
    let file = VaultPath::parse("x.txt")?;
    let locked = |r: Result<(), VaultError>| matches!(r, Err(VaultError::Locked));
    assert!(locked(vault.lock(LockPolicy::Force).map(|_| ())));
    assert!(locked(
        vault.change_password(&pw(PASSWORD), &pw(OTHER_PASSWORD))
    ));
    assert!(locked(vault.list(&root).map(|_| ())));
    assert!(locked(vault.create_dir(&file)));
    assert!(locked(vault.rename(&file, &file)));
    assert!(locked(vault.delete(&file)));
    assert!(locked(
        vault.import(&src, &root, Conflict::Fail, None).map(|_| ())
    ));
    assert!(locked(vault.export(&file, &scratch.join("out"), None)));
    assert!(locked(vault.open_in_session(&file, None).map(|_| ())));
    assert!(locked(vault.sync_session().map(|_| ())));
    assert!(locked(vault.import_foreign(None).map(|_| ())));
    assert!(locked(vault.verify(None).map(|_| ())));
    assert_eq!(vault.status().state, VaultState::Locked);
    Ok(())
}

#[test]
fn second_instance_is_busy_and_wipes_nothing() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.create()?;
    fixture.import_bytes(&first, "", "doc.txt", b"open me")?;
    let session_file = first.open_in_session(&VaultPath::parse("doc.txt")?, None)?;

    let second = Vault::open(fixture.config.clone());
    assert!(matches!(second, Err(VaultError::Busy)));
    assert!(
        session_file.exists(),
        "the first instance's session is untouched"
    );

    first.lock(LockPolicy::SyncThenWipe)?;
    drop(first);
    let reopened = fixture.open()?;
    assert_eq!(reopened.status().state, VaultState::Locked);
    Ok(())
}

#[test]
fn out_of_memory_kdf_returns_error_not_abort() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    vault.lock(LockPolicy::SyncThenWipe)?;

    // Inject an allocation limit far below the 19 MiB the KDF needs.
    vault.set_kdf_memory_limit(Some(1024 * 1024));
    let result = vault.unlock(&pw(PASSWORD));
    assert!(
        matches!(result, Err(VaultError::OutOfMemory { needed_mib: 19 })),
        "{result:?}"
    );
    let status = vault.status();
    assert_eq!(status.state, VaultState::Locked);
    assert_eq!(
        status.failed_attempts, 0,
        "an attempt that never ran the KDF is not counted"
    );

    vault.set_kdf_memory_limit(None);
    vault.unlock(&pw(PASSWORD))?;

    // create() fails the same way and leaves no vault behind.
    let other = Fixture::new()?;
    let fresh = other.open()?;
    fresh.set_kdf_memory_limit(Some(4096));
    assert!(matches!(
        fresh.create(&pw(PASSWORD)),
        Err(VaultError::OutOfMemory { .. })
    ));
    assert!(!other.root().join("vault.a.gvh").exists());
    assert_eq!(fresh.status().state, VaultState::Uninitialized);
    Ok(())
}

#[test]
fn debug_output_never_contains_password_or_key() -> TestResult {
    let secret_password = "hunter2-but-much-longer";
    let secret_name = "very-secret-name.txt";
    let fixture = Fixture::new()?;
    let vault = fixture.open()?;
    vault.create(&pw(secret_password))?;
    fixture.import_bytes(&vault, "", secret_name, b"payload")?;

    let kek = kdf::derive_kek(&pw(secret_password), &[1u8; 16], KdfParams::MINIMUM)?;
    let key_hex = common::hex(kek.expose_secret());
    let key_dec = format!("{:?}", kek.expose_secret());

    let wrong = vault.change_password(&pw("not-the-password-1"), &pw(secret_password));
    let errors = [
        VaultError::WrongPassword,
        VaultError::PasswordRejected("too short"),
        VaultError::Throttled {
            retry_after: std::time::Duration::from_secs(5),
        },
        VaultError::UnsyncedEdits(vec![VaultPath::parse(secret_name)?]),
    ];
    let mut outputs = vec![
        format!("{vault:?}"),
        format!("{:?}", fixture.config),
        format!("{:?}", vault.status()),
        format!("{:?}", pw(secret_password)),
        format!("{kek:?}"),
        format!("{wrong:?}"),
        format!("{:?}", KdfParams::STANDARD),
    ];
    for e in &errors {
        outputs.push(format!("{e:?}"));
        outputs.push(e.to_string());
    }
    for out in &outputs {
        assert!(!out.contains(secret_password), "password leaked: {out}");
        assert!(!out.contains(secret_name), "file name leaked: {out}");
        assert!(!out.contains(&key_hex), "key leaked: {out}");
        assert!(
            !out.contains(&key_dec[1..key_dec.len() - 1]),
            "key leaked: {out}"
        );
        assert!(
            !has_byte_list(out),
            "something that looks like key bytes leaked: {out}"
        );
    }
    Ok(())
}

/// Three or more comma-separated decimal numbers in a row, as `Debug` prints a byte array.
fn has_byte_list(s: &str) -> bool {
    let mut run = 0;
    for part in s.split(", ") {
        let digits = part.trim_matches(|c| c == '[' || c == ']');
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            run += 1;
            if run >= 3 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

#[test]
fn weak_kdf_is_upgraded_on_unlock_without_touching_data() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    fixture.import_bytes(&vault, "", "a.txt", b"alpha")?;
    vault.lock(LockPolicy::SyncThenWipe)?;
    drop(vault);
    let mut data_before = snapshot(fixture.root())?;
    for name in ["vault.a.gvh", "vault.b.gvh", "vault.guard", "vault.lock"] {
        data_before.remove(std::path::Path::new(name));
    }

    let stronger = KdfParams {
        m_cost_kib: 32_768,
        t_cost: 2,
        p_cost: 1,
    };
    let config = VaultConfig {
        kdf: stronger,
        ..fixture.config.clone()
    };
    let vault = Vault::open(config)?.0;
    assert!(vault.status().kdf_upgrade_pending);
    vault.unlock(&pw(PASSWORD))?;
    let status = vault.status();
    assert!(!status.kdf_upgrade_pending);
    assert_eq!(status.kdf, Some(stronger));
    let header = HeaderSlot::decode(&fs::read(fixture.root().join("vault.b.gvh"))?)?;
    assert_eq!(header.kdf, stronger);
    assert_eq!(header.generation, 2);

    let mut data_after = snapshot(fixture.root())?;
    for name in ["vault.a.gvh", "vault.b.gvh", "vault.guard", "vault.lock"] {
        data_after.remove(std::path::Path::new(name));
    }
    assert_eq!(data_before, data_after);
    vault.lock(LockPolicy::SyncThenWipe)?;
    vault.unlock(&pw(PASSWORD))?;
    assert_eq!(fixture.export_bytes(&vault, "a.txt")?, b"alpha");
    Ok(())
}

#[test]
fn create_dir_delete_and_verify() -> TestResult {
    let fixture = Fixture::new()?;
    let vault = fixture.create()?;
    vault.create_dir(&VaultPath::parse("Taxes")?)?;
    fixture.import_bytes(&vault, "Taxes", "2025.pdf", b"%PDF")?;
    fixture.import_bytes(&vault, "", "keep.txt", b"keep")?;
    assert_eq!(fixture.blobs()?.len(), 2);
    assert!(matches!(
        vault.create_dir(&VaultPath::parse("Missing/Child")?),
        Err(VaultError::NotFound(_))
    ));

    let report = vault.verify(None)?;
    assert_eq!(report.files_checked, 2);
    common::assert_empty(&report.problems);
    assert_eq!(report.orphan_blobs, 0);

    vault.delete(&VaultPath::parse("Taxes")?)?;
    assert_eq!(common::names(&vault, "")?, vec!["keep.txt".to_owned()]);
    assert_eq!(
        fixture.blobs()?.len(),
        1,
        "the deleted file's blob is removed"
    );
    assert!(matches!(
        vault.delete(&VaultPath::parse("Taxes")?),
        Err(VaultError::NotFound(_))
    ));
    Ok(())
}

//! Golden vectors and known answers (research/vault.md Appendix A, tests F1 to F5, H5).

// cspell:ignore gsvaulx nocapture

mod common;

use std::time::Instant;

use common::{Fixture, PASSWORD, TestResult, hex, parse_hex, pw};
use genslate_vault::format::blob::{BLOB_HEADER_LEN, BlobHeader, encrypted_len};
use genslate_vault::format::header::{HEADER_LEN, HeaderSlot};
use genslate_vault::{ExposeSecret, KdfParams, SecretBox, VaultConfig, VaultError, crypto, kdf};

const KAT_SALT: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

const GOLDEN_HEADER: &str = "
    47535641554c5400 0100 0100 0100000000000000
    a0a1a2a3a4a5a6a7a8a9aaabacadaeaf 01 13 0000 08000000 01000000 01000000
    000102030405060708090a0b0c0d0e0f
    101112131415161718191a1b1c1d1e1f2021222324252627
    c36be9aa3c00ad31c92f39d75ab13faed6402c12f6bed65309a612ae0c670fea
    0ae8fd34a82bbee37fa95af07486590e
    e177dd1d7376253fdecdb4ce9bd10dea";

const GOLDEN_BLOB_HEADER: &str = "
    47535646494c4500 0100 0000 00000100 b0b1b2b3b4b5b6b7b8b9babbbcbdbebf
    707172737475767778797a7b7c7d7e7f808182 00000000000000000000000000";

const GOLDEN_HELLO_CHUNK: &str = "602f1575b8fd62ece2df22d556e146c47e7a625005";

fn seq<const N: usize>(start: u8) -> [u8; N] {
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = start.wrapping_add(u8::try_from(i).unwrap_or(0));
    }
    out
}

fn vault_key() -> SecretBox<[u8; 32]> {
    SecretBox::new(Box::new(seq::<32>(0x40)))
}

#[test]
fn argon2id_known_answers_match_reference() -> TestResult {
    let cases = [
        (
            KdfParams {
                m_cost_kib: 8,
                t_cost: 1,
                p_cost: 1,
            },
            "d17ea6341ca93da6079ea2f64dc4aa31dd1aaf9caa67fb42ac4afd0714706f26",
        ),
        (
            KdfParams {
                m_cost_kib: 19_456,
                t_cost: 2,
                p_cost: 1,
            },
            "818259b6310026a8e0dbac5d2e6927abcfdb07b32258fac4f61b18b80f929085",
        ),
    ];
    for (params, expected) in cases {
        let kek = kdf::derive_kek(&pw(PASSWORD), &KAT_SALT, params)?;
        assert_eq!(hex(kek.expose_secret()), expected, "KAT for {params:?}");
    }
    Ok(())
}

#[test]
fn header_slot_golden_vector() -> TestResult {
    let params = KdfParams {
        m_cost_kib: 8,
        t_cost: 1,
        p_cost: 1,
    };
    let kek = kdf::derive_kek(&pw(PASSWORD), &KAT_SALT, params)?;
    let mut slot = HeaderSlot {
        generation: 1,
        vault_id: seq::<16>(0xa0),
        kdf: params,
        salt: KAT_SALT,
        wrap_nonce: seq::<24>(0x10),
        wrapped_vk: [0u8; 48],
    };
    slot.wrapped_vk =
        crypto::wrap_vault_key(&kek, &slot.wrap_nonce, &vault_key(), &slot.wrap_aad())?;
    let encoded = slot.encode();
    assert_eq!(encoded.len(), HEADER_LEN);
    assert_eq!(HEADER_LEN, 156);
    assert_eq!(hex(&encoded), hex(&parse_hex(GOLDEN_HEADER)));

    let parsed = HeaderSlot::decode(&parse_hex(GOLDEN_HEADER))?;
    assert_eq!(parsed, slot);
    let vk = crypto::unwrap_vault_key(
        &kek,
        &parsed.wrap_nonce,
        &parsed.wrapped_vk,
        &parsed.wrap_aad(),
    )?;
    assert_eq!(vk.expose_secret(), vault_key().expose_secret());
    Ok(())
}

#[test]
fn header_wrong_kek_is_wrong_password() -> TestResult {
    let parsed = HeaderSlot::decode(&parse_hex(GOLDEN_HEADER))?;
    let wrong = kdf::derive_kek(&pw("not the password"), &KAT_SALT, parsed.kdf)?;
    let result = crypto::unwrap_vault_key(
        &wrong,
        &parsed.wrap_nonce,
        &parsed.wrapped_vk,
        &parsed.wrap_aad(),
    );
    assert!(matches!(result, Err(VaultError::WrongPassword)));
    Ok(())
}

#[test]
fn header_field_bounds_are_enforced() {
    let golden = parse_hex(GOLDEN_HEADER);
    // Each mutation re-computes the checksum so only the field rule can reject it.
    let reseal = |mut bytes: Vec<u8>| -> Vec<u8> {
        let check = blake2b16(&bytes[..140]);
        bytes[140..156].copy_from_slice(&check);
        bytes
    };
    let with = |offset: usize, value: &[u8]| -> Vec<u8> {
        let mut bytes = golden.clone();
        bytes[offset..offset + value.len()].copy_from_slice(value);
        reseal(bytes)
    };

    // Damage without a matching checksum is HeaderDamaged.
    let mut flipped = golden.clone();
    flipped[30] ^= 1;
    assert!(matches!(
        HeaderSlot::decode(&flipped),
        Err(VaultError::HeaderDamaged)
    ));
    assert!(matches!(
        HeaderSlot::decode(&golden[..155]),
        Err(VaultError::HeaderDamaged)
    ));
    assert!(matches!(
        HeaderSlot::decode(&with(0, b"GSVAULX\0")),
        Err(VaultError::HeaderDamaged)
    ));

    assert!(matches!(
        HeaderSlot::decode(&with(8, &2u16.to_le_bytes())),
        Err(VaultError::UnsupportedVersion(2))
    ));
    assert!(matches!(
        HeaderSlot::decode(&with(10, &3u16.to_le_bytes())),
        Err(VaultError::UnsupportedVersion(_))
    ));
    assert!(matches!(
        HeaderSlot::decode(&with(36, &[2])),
        Err(VaultError::UnsupportedKdf)
    ));
    assert!(matches!(
        HeaderSlot::decode(&with(37, &[0x10])),
        Err(VaultError::UnsupportedKdf)
    ));
    assert!(
        HeaderSlot::decode(&with(38, &[1, 0])).is_err(),
        "non-zero reserved"
    );
    // m below 8, above 1 GiB, t = 0, t > 64, p = 0, p > 16, m < 8p.
    for (offset, value) in [
        (40, 7u32),
        (40, 1_048_577),
        (44, 0),
        (44, 65),
        (48, 0),
        (48, 17),
        (48, 2), // m = 8 < 8 * 2
    ] {
        let result = HeaderSlot::decode(&with(offset, &value.to_le_bytes()));
        assert!(
            matches!(result, Err(VaultError::UnsupportedKdf)),
            "offset {offset} value {value}"
        );
    }
    // Each field in bounds, but together too much work for one unlock: refused before the KDF.
    let costs = |m: u32, t: u32| {
        let mut bytes = golden.clone();
        bytes[40..44].copy_from_slice(&m.to_le_bytes());
        bytes[44..48].copy_from_slice(&t.to_le_bytes());
        HeaderSlot::decode(&reseal(bytes))
    };
    assert!(matches!(
        costs(1_048_576, 64),
        Err(VaultError::UnsupportedKdf)
    ));
    assert!(matches!(
        costs(1_048_576, 5),
        Err(VaultError::UnsupportedKdf)
    ));
    assert!(matches!(
        costs(262_144, 17),
        Err(VaultError::UnsupportedKdf)
    ));
    assert!(
        costs(1_048_576, 4).is_ok(),
        "the work cap itself is allowed"
    );
    assert!(costs(65_536, 64).is_ok());
}

#[test]
fn kdf_read_bounds_reject_hostile_parameters_without_allocating() {
    let huge = KdfParams {
        m_cost_kib: u32::MAX,
        t_cost: 1,
        p_cost: 1,
    };
    assert!(matches!(
        huge.validate_for_read(),
        Err(VaultError::UnsupportedKdf)
    ));
    assert!(matches!(
        kdf::derive_kek(&pw(PASSWORD), &KAT_SALT, huge),
        Err(VaultError::UnsupportedKdf)
    ));
    assert!(KdfParams::STANDARD.validate_for_read().is_ok());
    assert_eq!(
        KdfParams::STANDARD,
        KdfParams {
            m_cost_kib: 131_072,
            t_cost: 3,
            p_cost: 1
        }
    );
}

#[test]
fn blob_golden_vector() -> TestResult {
    let header = BlobHeader {
        chunk_size: 65_536,
        blob_id: seq::<16>(0xb0),
        stream_nonce: seq::<19>(0x70),
    };
    assert_eq!(hex(&header.encode()), hex(&parse_hex(GOLDEN_BLOB_HEADER)));
    assert_eq!(BlobHeader::decode(&parse_hex(GOLDEN_BLOB_HEADER))?, header);

    let vault_id = seq::<16>(0xa0);
    let mut sealed = Vec::new();
    let plain_len =
        crypto::encrypt_blob(&vault_key(), &vault_id, &header, &b"hello"[..], &mut sealed)?;
    assert_eq!(plain_len, 5);
    assert_eq!(sealed.len(), BLOB_HEADER_LEN + 21);
    assert_eq!(
        hex(&sealed[..BLOB_HEADER_LEN]),
        hex(&parse_hex(GOLDEN_BLOB_HEADER))
    );
    assert_eq!(hex(&sealed[BLOB_HEADER_LEN..]), GOLDEN_HELLO_CHUNK);

    let mut plain = Vec::new();
    let len = u64::try_from(sealed.len())?;
    crypto::decrypt_blob(
        &vault_key(),
        &vault_id,
        &header.blob_id,
        &sealed[..],
        len,
        &mut plain,
    )?;
    assert_eq!(plain, b"hello");

    // Wrong vault id (cross-vault blob) and wrong expected id both fail.
    let other_vault = seq::<16>(0x01);
    let result = crypto::decrypt_blob(
        &vault_key(),
        &other_vault,
        &header.blob_id,
        &sealed[..],
        len,
        &mut Vec::new(),
    );
    assert!(matches!(result, Err(VaultError::Tampered(_))));
    let result = crypto::decrypt_blob(
        &vault_key(),
        &vault_id,
        &seq::<16>(0xc0),
        &sealed[..],
        len,
        &mut Vec::new(),
    );
    assert!(matches!(result, Err(VaultError::Tampered(_))));
    Ok(())
}

#[test]
fn blob_length_math_and_round_trip() -> TestResult {
    let vault_id = [7u8; 16];
    for chunk_size in [4096u32, 65_536] {
        let c = usize::try_from(chunk_size)?;
        for len in [0, 1, c - 1, c, c + 1, 2 * c, 3 * c + 17] {
            let header = BlobHeader {
                chunk_size,
                blob_id: [9u8; 16],
                stream_nonce: [3u8; 19],
            };
            let plain = common::pseudo_random(len, u64::try_from(len)?);
            let mut sealed = Vec::new();
            crypto::encrypt_blob(&vault_key(), &vault_id, &header, &plain[..], &mut sealed)?;
            let chunks = len.div_ceil(c).max(1);
            assert_eq!(sealed.len(), 64 + len + 16 * chunks, "L = {len}, C = {c}");
            assert_eq!(
                encrypted_len(u64::try_from(len)?, chunk_size),
                u64::try_from(sealed.len())?
            );
            let mut out = Vec::new();
            let n = crypto::decrypt_blob(
                &vault_key(),
                &vault_id,
                &header.blob_id,
                &sealed[..],
                u64::try_from(sealed.len())?,
                &mut out,
            )?;
            assert_eq!(n, u64::try_from(len)?);
            assert_eq!(out, plain);
        }
    }
    Ok(())
}

/// F3: the 128 MiB known answer for the production parameters.
#[test]
fn standard_kdf_known_answer() -> TestResult {
    let kek = kdf::derive_kek(&pw(PASSWORD), &KAT_SALT, KdfParams::STANDARD)?;
    assert_eq!(
        hex(kek.expose_secret()),
        "04bcda12921de9eb356711dc2c45129dc5f56b330f232cbbb683f5dd83a0736c"
    );
    Ok(())
}

/// H5: the production KDF budget, measured on the machine it runs on. A wall-clock check, so
/// it is not part of the default run: `cargo test -p genslate-vault -- --ignored --nocapture`.
#[test]
#[ignore = "wall-clock timing; run with --ignored to measure the STANDARD unlock"]
#[allow(clippy::print_stderr)] // the measured time is part of the task report
fn standard_kdf_unlock_is_under_three_seconds() -> TestResult {
    let fixture = Fixture::new()?;
    let config = VaultConfig {
        kdf: KdfParams::STANDARD,
        ..fixture.config.clone()
    };
    let vault = genslate_vault::Vault::open(config)?.0;
    vault.create(&pw(PASSWORD))?;
    vault.lock(genslate_vault::LockPolicy::SyncThenWipe)?;
    let started = Instant::now();
    vault.unlock(&pw(PASSWORD))?;
    let elapsed = started.elapsed();
    eprintln!("STANDARD (m=131072 KiB, t=3, p=1) unlock took {elapsed:?}");
    assert!(elapsed.as_secs_f64() < 3.0, "unlock took {elapsed:?}");
    Ok(())
}

fn blake2b16(data: &[u8]) -> [u8; 16] {
    use blake2::Digest as _;
    let mut hasher = blake2::Blake2b::<blake2::digest::consts::U16>::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut arr = [0u8; 16];
    arr.copy_from_slice(&out);
    arr
}

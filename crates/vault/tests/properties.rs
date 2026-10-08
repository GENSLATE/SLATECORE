//! Property tests for the blob length rules (research/vault.md F5, proptest over L and C).

mod common;

use genslate_vault::format::blob::{
    BLOB_HEADER_LEN, BlobHeader, BlobLayout, TAG_LEN, encrypted_len,
};
use genslate_vault::{SecretBox, crypto};
use proptest::prelude::*;

const TAG: u64 = TAG_LEN as u64;
const HEAD: u64 = BLOB_HEADER_LEN as u64;

fn fail(error: impl std::fmt::Display) -> TestCaseError {
    TestCaseError::fail(error.to_string())
}

/// A remainder after the full sealed chunks: biased towards the tag-sized edge cases.
fn remainder(chunk_size: u32) -> impl Strategy<Value = u64> {
    let sealed = u64::from(chunk_size) + TAG;
    prop_oneof![0..=2 * TAG, 0..sealed]
}

proptest! {
    /// A writer's length always parses back to the same plaintext length and chunk count.
    #[test]
    fn encrypted_length_parses_back(exp in 12u32..=20, len in 0u64..(1 << 40)) {
        let chunk_size = 1u32 << exp;
        let file_len = encrypted_len(len, chunk_size);
        let chunks = len.div_ceil(u64::from(chunk_size)).max(1);
        prop_assert_eq!(file_len, HEAD + len + TAG * chunks);
        let layout = BlobLayout::from_file_len(file_len, chunk_size).map_err(fail)?;
        prop_assert_eq!(layout, BlobLayout { chunks, plain_len: len });
    }

    /// Every length the reader accepts is one a writer produces; all others are rejected.
    #[test]
    fn accepted_lengths_are_exactly_writer_lengths(
        (exp, full, rem) in (12u32..=20)
            .prop_flat_map(|exp| (Just(exp), 0u64..4096, remainder(1 << exp))),
    ) {
        let chunk_size = 1u32 << exp;
        let sealed = u64::from(chunk_size) + TAG;
        let file_len = HEAD + full * sealed + rem;
        if let Ok(layout) = BlobLayout::from_file_len(file_len, chunk_size) {
            prop_assert_eq!(encrypted_len(layout.plain_len, chunk_size), file_len);
        } else {
            // Rejected: no plaintext length may encrypt to this size. A writer's blob of
            // k chunks has `full` equal to k - 1 or k, so k is `full` or `full + 1`.
            let body = file_len - HEAD;
            for chunks in [full.max(1), full + 1] {
                if let Some(candidate) = body.checked_sub(TAG * chunks) {
                    prop_assert_ne!(encrypted_len(candidate, chunk_size), file_len);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Encrypting then decrypting any length at any small chunk size returns the input.
    #[test]
    fn blob_round_trip(exp in 12u32..=14, fraction in 0u64..=1000, seed in any::<u64>()) {
        let chunk_size = 1u32 << exp;
        let max = 3 * u64::from(chunk_size) + 17;
        let len = usize::try_from(max * fraction / 1000).map_err(fail)?;
        let plain = common::pseudo_random(len, seed);
        let header = BlobHeader { chunk_size, blob_id: [5u8; 16], stream_nonce: [6u8; 19] };
        let key = SecretBox::new(Box::new([0x42u8; 32]));
        let vault_id = [8u8; 16];
        let mut sealed = Vec::new();
        crypto::encrypt_blob(&key, &vault_id, &header, &plain[..], &mut sealed).map_err(fail)?;
        let sealed_len = u64::try_from(sealed.len()).map_err(fail)?;
        prop_assert_eq!(sealed_len, encrypted_len(u64::try_from(len).map_err(fail)?, chunk_size));
        let mut out = Vec::new();
        crypto::decrypt_blob(&key, &vault_id, &header.blob_id, &sealed[..], sealed_len, &mut out)
            .map_err(fail)?;
        prop_assert_eq!(out, plain);
    }
}

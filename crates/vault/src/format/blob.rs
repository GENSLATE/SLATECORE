//! Encrypted file (blob) `files/<32 hex>.gvf` (research/vault.md 4.3).
//!
//! A 64-byte header, then `n = max(1, ceil(L / C))` sealed chunks of a plaintext of `L` bytes
//! with chunk size `C`, each ciphertext followed by a 16-byte Poly1305 tag. Chunk `i` uses the
//! nonce `stream_nonce(19) || u32_BE(i) || last_flag(1)` (aead-stream `StreamBE32`) and the
//! associated data `"GSV1-BLOB" || vault_id || header(64)`.
//!
//! | Offset | Len | Field |
//! |---|---|---|
//! | 0 | 8 | magic `GSVFILE\0` |
//! | 8 | 2 | format version (1) |
//! | 10 | 2 | flags (0) |
//! | 12 | 4 | chunk size (power of two, 4096 to 1048576; writer uses 65536) |
//! | 16 | 16 | blob id (equals the file name and the index entry) |
//! | 32 | 19 | stream nonce prefix |
//! | 51 | 13 | reserved, zero |

// cspell:ignore gsvfile

use super::{FORMAT_VERSION, array, u16_at, u32_at};
use crate::error::{VaultError, tamper};

/// Size of the blob header.
pub const BLOB_HEADER_LEN: usize = 64;
/// `GSVFILE\0`.
pub const MAGIC: [u8; 8] = *b"GSVFILE\0";
/// Plaintext bytes per full chunk written by this crate.
pub const DEFAULT_CHUNK_SIZE: u32 = 65_536;
/// Poly1305 tag length.
pub const TAG_LEN: usize = 16;

const MIN_CHUNK: u32 = 4096;
const MAX_CHUNK: u32 = 1_048_576;
const BLOB_LABEL: &[u8] = b"GSV1-BLOB";

/// The 64-byte blob header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlobHeader {
    pub chunk_size: u32,
    pub blob_id: [u8; 16],
    pub stream_nonce: [u8; 19],
}

impl BlobHeader {
    /// Serialises the header.
    pub fn encode(&self) -> [u8; BLOB_HEADER_LEN] {
        let mut b = [0u8; BLOB_HEADER_LEN];
        b[0..8].copy_from_slice(&MAGIC);
        b[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        b[12..16].copy_from_slice(&self.chunk_size.to_le_bytes());
        b[16..32].copy_from_slice(&self.blob_id);
        b[32..51].copy_from_slice(&self.stream_nonce);
        b
    }

    /// Parses and checks the header fields (the AEAD checks the rest).
    pub fn decode(bytes: &[u8]) -> Result<Self, VaultError> {
        if bytes.len() < BLOB_HEADER_LEN {
            return Err(VaultError::Tampered(tamper::TRUNCATED));
        }
        if bytes[0..8] != MAGIC {
            return Err(VaultError::Tampered(tamper::BLOB_HEADER));
        }
        let version = u16_at(bytes, 8);
        if version != FORMAT_VERSION || u16_at(bytes, 10) != 0 {
            return Err(VaultError::UnsupportedVersion(version));
        }
        let chunk_size = u32_at(bytes, 12);
        if !chunk_size.is_power_of_two() || !(MIN_CHUNK..=MAX_CHUNK).contains(&chunk_size) {
            return Err(VaultError::Tampered(tamper::BLOB_HEADER));
        }
        if bytes[51..BLOB_HEADER_LEN].iter().any(|b| *b != 0) {
            return Err(VaultError::Tampered(tamper::BLOB_HEADER));
        }
        Ok(Self {
            chunk_size,
            blob_id: array(bytes, 16),
            stream_nonce: array(bytes, 32),
        })
    }

    /// Associated data for every chunk: `"GSV1-BLOB" || vault_id || header(64)`.
    pub fn aad(&self, vault_id: &[u8; 16]) -> Vec<u8> {
        let mut aad = Vec::with_capacity(BLOB_LABEL.len() + 16 + BLOB_HEADER_LEN);
        aad.extend_from_slice(BLOB_LABEL);
        aad.extend_from_slice(vault_id);
        aad.extend_from_slice(&self.encode());
        aad
    }
}

/// Encrypted file length for a plaintext of `plain_len` bytes:
/// `64 + L + 16 * max(1, ceil(L / C))` (saturating).
pub fn encrypted_len(plain_len: u64, chunk_size: u32) -> u64 {
    let chunks = plain_len.div_ceil(u64::from(chunk_size.max(1))).max(1);
    (BLOB_HEADER_LEN as u64)
        .saturating_add(plain_len)
        .saturating_add(chunks.saturating_mul(TAG_LEN as u64))
}

/// Chunk structure derived from an encrypted file's length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlobLayout {
    /// Number of sealed chunks (at least 1).
    pub chunks: u64,
    /// Plaintext length.
    pub plain_len: u64,
}

impl BlobLayout {
    /// Validates the length of an encrypted file and derives its chunk count and plaintext
    /// length. Any length a writer could not have produced is [`VaultError::Tampered`].
    pub fn from_file_len(file_len: u64, chunk_size: u32) -> Result<Self, VaultError> {
        let truncated = VaultError::Tampered(tamper::TRUNCATED);
        let tag = TAG_LEN as u64;
        let body = file_len
            .checked_sub(BLOB_HEADER_LEN as u64)
            .ok_or(truncated)?;
        if body < tag {
            return Err(VaultError::Tampered(tamper::TRUNCATED));
        }
        let sealed_chunk = u64::from(chunk_size) + tag;
        let full = body / sealed_chunk;
        let rem = body % sealed_chunk;
        let chunks = if rem == 0 {
            full
        } else {
            // A short last chunk holds 1..C-1 bytes; an empty one only when it is the only one.
            if rem < tag || (rem == tag && full > 0) {
                return Err(VaultError::Tampered(tamper::TRUNCATED));
            }
            full + 1
        };
        // The STREAM counter is 32 bits.
        if chunks == 0 || chunks - 1 > u64::from(u32::MAX) {
            return Err(VaultError::Tampered(tamper::TRUNCATED));
        }
        Ok(Self {
            chunks,
            plain_len: body - chunks * tag,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_inverts_encrypted_len() -> Result<(), VaultError> {
        for c in [4096u32, 65_536] {
            let c64 = u64::from(c);
            for len in [0, 1, c64 - 1, c64, c64 + 1, 2 * c64, 3 * c64 + 17] {
                let layout = BlobLayout::from_file_len(encrypted_len(len, c), c)?;
                assert_eq!(layout.plain_len, len);
                assert_eq!(layout.chunks, len.div_ceil(c64).max(1));
            }
        }
        Ok(())
    }

    #[test]
    fn impossible_lengths_are_rejected() {
        let c = 4096u32;
        let sealed = u64::from(c) + 16;
        for bad in [0, 63, 64, 79, 64 + sealed + 15, 64 + sealed + 16] {
            assert!(BlobLayout::from_file_len(bad, c).is_err(), "{bad}");
        }
        assert!(BlobLayout::from_file_len(80, c).is_ok(), "empty file");
    }
}

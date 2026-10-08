//! Header slot `vault.{a,b}.gvh`, 156 bytes (research/vault.md 4.2).
//!
//! | Offset | Len | Field |
//! |---|---|---|
//! | 0 | 8 | magic `GSVAULT\0` |
//! | 8 | 2 | format version (1) |
//! | 10 | 2 | flags (bit 0: password was NFKC-normalised; always set in v1) |
//! | 12 | 8 | generation |
//! | 20 | 16 | vault id |
//! | 36 | 1 | KDF algorithm (1 = Argon2id) |
//! | 37 | 1 | KDF version (0x13) |
//! | 38 | 2 | reserved, zero |
//! | 40 | 12 | m (KiB), t, p |
//! | 52 | 16 | salt |
//! | 68 | 24 | wrap nonce |
//! | 92 | 48 | wrapped vault key (32 + 16-byte tag) |
//! | 140 | 16 | `BLAKE2b-128` of bytes `[0, 140)` (corruption check, not security) |

use blake2::digest::consts::U16;
use blake2::{Blake2b, Digest};

use super::{FORMAT_VERSION, array, u16_at, u32_at, u64_at};
use crate::error::VaultError;
use crate::kdf::KdfParams;

/// Size of one header slot.
pub const HEADER_LEN: usize = 156;
/// `GSVAULT\0`.
pub const MAGIC: [u8; 8] = *b"GSVAULT\0";
/// Header flag: the password was NFKC-normalised before the KDF.
pub const FLAG_NFKC: u16 = 1;
/// KDF algorithm id for Argon2id.
pub const KDF_ARGON2ID: u8 = 1;
/// Argon2 version 1.3.
pub const ARGON2_VERSION: u8 = 0x13;

const WRAP_LABEL: &[u8] = b"GSV1-WRAP";
const AAD_PREFIX: usize = 68;
const CHECK_AT: usize = 140;

/// One parsed header slot. Format version and flags are fixed in v1 and therefore not fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderSlot {
    pub generation: u64,
    pub vault_id: [u8; 16],
    pub kdf: KdfParams,
    pub salt: [u8; 16],
    pub wrap_nonce: [u8; 24],
    pub wrapped_vk: [u8; 48],
}

impl HeaderSlot {
    /// Serialises the slot, computing the checksum.
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        b[0..8].copy_from_slice(&MAGIC);
        b[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        b[10..12].copy_from_slice(&FLAG_NFKC.to_le_bytes());
        b[12..20].copy_from_slice(&self.generation.to_le_bytes());
        b[20..36].copy_from_slice(&self.vault_id);
        b[36] = KDF_ARGON2ID;
        b[37] = ARGON2_VERSION;
        b[40..44].copy_from_slice(&self.kdf.m_cost_kib.to_le_bytes());
        b[44..48].copy_from_slice(&self.kdf.t_cost.to_le_bytes());
        b[48..52].copy_from_slice(&self.kdf.p_cost.to_le_bytes());
        b[52..68].copy_from_slice(&self.salt);
        b[68..92].copy_from_slice(&self.wrap_nonce);
        b[92..140].copy_from_slice(&self.wrapped_vk);
        let check = checksum(&b[..CHECK_AT]);
        b[CHECK_AT..].copy_from_slice(&check);
        b
    }

    /// Parses and validates one slot.
    ///
    /// Wrong length, magic or checksum: [`VaultError::HeaderDamaged`]. Unknown version, flags or
    /// reserved bits: [`VaultError::UnsupportedVersion`]. Unknown KDF or parameters outside the
    /// read bounds: [`VaultError::UnsupportedKdf`].
    pub fn decode(bytes: &[u8]) -> Result<Self, VaultError> {
        if bytes.len() != HEADER_LEN || bytes[0..8] != MAGIC {
            return Err(VaultError::HeaderDamaged);
        }
        if checksum(&bytes[..CHECK_AT]) != bytes[CHECK_AT..] {
            return Err(VaultError::HeaderDamaged);
        }
        let version = u16_at(bytes, 8);
        if version != FORMAT_VERSION || u16_at(bytes, 10) != FLAG_NFKC || u16_at(bytes, 38) != 0 {
            return Err(VaultError::UnsupportedVersion(version));
        }
        if bytes[36] != KDF_ARGON2ID || bytes[37] != ARGON2_VERSION {
            return Err(VaultError::UnsupportedKdf);
        }
        let kdf = KdfParams {
            m_cost_kib: u32_at(bytes, 40),
            t_cost: u32_at(bytes, 44),
            p_cost: u32_at(bytes, 48),
        }
        .validate_for_read()?;
        Ok(Self {
            generation: u64_at(bytes, 12),
            vault_id: array(bytes, 20),
            kdf,
            salt: array(bytes, 52),
            wrap_nonce: array(bytes, 68),
            wrapped_vk: array(bytes, 92),
        })
    }

    /// Associated data of the key wrap: `"GSV1-WRAP" || bytes[0, 68)`, binding version, flags,
    /// generation, vault id, KDF and salt to the wrapped key.
    pub fn wrap_aad(&self) -> Vec<u8> {
        let encoded = self.encode();
        let mut aad = Vec::with_capacity(WRAP_LABEL.len() + AAD_PREFIX);
        aad.extend_from_slice(WRAP_LABEL);
        aad.extend_from_slice(&encoded[..AAD_PREFIX]);
        aad
    }
}

/// `BLAKE2b` with a 16-byte digest (`digest_size = 16`, not a truncated 64-byte digest).
fn checksum(data: &[u8]) -> [u8; 16] {
    let mut hasher = Blake2b::<U16>::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest);
    out
}

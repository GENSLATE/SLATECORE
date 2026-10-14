//! Password handling and the Argon2id key-encryption key (research/vault.md 2).

use argon2::{Algorithm, Argon2, Block, Params, Version};
use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox, SecretString};
use unicode_normalization::UnicodeNormalization;
use zeroize::{Zeroize, Zeroizing};

use crate::error::VaultError;

/// Shortest accepted password, in UTF-8 bytes after NFKC normalisation.
pub const MIN_PASSWORD_BYTES: usize = 8;
/// Longest accepted password, in UTF-8 bytes after NFKC normalisation.
pub const MAX_PASSWORD_BYTES: usize = 1024;

/// Largest memory cost a header may ask for, in KiB (1 GiB).
const MAX_M_COST_KIB: u32 = 1_048_576;
/// Largest total work a header may ask for, as memory (KiB) times passes: 1 GiB for 4 passes,
/// about ten times [`KdfParams::STANDARD`]. Every field can be in bounds while the product would
/// still keep a PC busy for minutes per attempt.
const MAX_WORK_KIB_PASSES: u64 = 4 * MAX_M_COST_KIB as u64;
/// The KDF memory cap when the launcher sets none: the largest header the format accepts.
pub(crate) const DEFAULT_MEMORY_LIMIT_BYTES: u64 = MAX_M_COST_KIB as u64 * 1024;

/// Argon2id cost parameters, stored in the vault header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// The default for new vaults: 128 MiB, 3 passes, 1 lane.
    pub const STANDARD: KdfParams = KdfParams {
        m_cost_kib: 131_072,
        t_cost: 3,
        p_cost: 1,
    };

    /// The weakest parameters a new vault may use (OWASP minimum: 19 MiB, 2 passes). Meant for
    /// tests and very small PCs; [`KdfParams::STANDARD`] stays the default.
    pub const MINIMUM: KdfParams = KdfParams {
        m_cost_kib: 19_456,
        t_cost: 2,
        p_cost: 1,
    };

    /// Hard bounds applied to parameters read from a header, so a hostile header cannot make
    /// the launcher allocate or compute without limit: `8 <= m <= 1 GiB`, `1 <= t <= 64`,
    /// `1 <= p <= 16`, `m >= 8p`, and `m * t <= 4 GiB` (KiB times passes).
    pub fn validate_for_read(self) -> Result<Self, VaultError> {
        let ok = (8..=MAX_M_COST_KIB).contains(&self.m_cost_kib)
            && (1..=64).contains(&self.t_cost)
            && (1..=16).contains(&self.p_cost)
            && u64::from(self.m_cost_kib) >= 8 * u64::from(self.p_cost)
            && u64::from(self.m_cost_kib) * u64::from(self.t_cost) <= MAX_WORK_KIB_PASSES;
        if ok {
            Ok(self)
        } else {
            Err(VaultError::UnsupportedKdf)
        }
    }

    /// The read bounds plus the creation floor ([`KdfParams::MINIMUM`]).
    pub fn validate_for_create(self) -> Result<Self, VaultError> {
        let params = self.validate_for_read()?;
        if params.m_cost_kib < Self::MINIMUM.m_cost_kib || params.t_cost < Self::MINIMUM.t_cost {
            return Err(VaultError::UnsupportedKdf);
        }
        Ok(params)
    }

    /// `true` when `self` costs less memory or fewer passes than `target`.
    pub(crate) fn is_weaker_than(self, target: KdfParams) -> bool {
        self.m_cost_kib < target.m_cost_kib || self.t_cost < target.t_cost
    }

    /// The parameters an unlock-time upgrade moves to: never lower than what is stored.
    pub(crate) fn upgraded_to(self, target: KdfParams) -> KdfParams {
        KdfParams {
            m_cost_kib: self.m_cost_kib.max(target.m_cost_kib),
            t_cost: self.t_cost.max(target.t_cost),
            p_cost: target.p_cost,
        }
    }

    /// Approximate memory in MiB, for [`VaultError::OutOfMemory`].
    pub(crate) fn needed_mib(self) -> u32 {
        self.m_cost_kib.div_ceil(1024)
    }
}

/// NFKC-normalises `password` and checks its length (8 to 1024 bytes).
///
/// The result is built into a buffer sized for the longest accepted password so it is never
/// reallocated (a reallocation would leave a copy behind that is never wiped). Buffers inside
/// the Unicode normaliser are not wiped (best effort, research/vault.md 2.6).
pub(crate) fn normalize_password(password: &SecretString) -> Result<Zeroizing<String>, VaultError> {
    let mut out = Zeroizing::new(String::with_capacity(MAX_PASSWORD_BYTES + 4));
    for c in password.expose_secret().nfkc() {
        if out.len() + c.len_utf8() > MAX_PASSWORD_BYTES {
            return Err(VaultError::PasswordRejected(
                "the password must be at most 1024 bytes",
            ));
        }
        out.push(c);
    }
    if out.len() < MIN_PASSWORD_BYTES {
        return Err(VaultError::PasswordRejected(
            "the password must be at least 8 bytes",
        ));
    }
    Ok(out)
}

/// Derives the 32-byte key-encryption key with Argon2id (version 0x13).
///
/// The password is NFKC-normalised and length-checked first; `params` must pass
/// [`KdfParams::validate_for_read`].
pub fn derive_kek(
    password: &SecretString,
    salt: &[u8; 16],
    params: KdfParams,
) -> Result<SecretBox<[u8; 32]>, VaultError> {
    let normalized = normalize_password(password)?;
    derive_kek_limited(normalized.as_bytes(), salt, params, memory_limit(None))
}

/// The KDF memory cap in bytes: what the launcher asked for, never above
/// [`DEFAULT_MEMORY_LIMIT_BYTES`].
pub(crate) fn memory_limit(requested: Option<u64>) -> u64 {
    requested.map_or(DEFAULT_MEMORY_LIMIT_BYTES, |bytes| {
        bytes.min(DEFAULT_MEMORY_LIMIT_BYTES)
    })
}

/// The Argon2 working memory, wiped on drop (argon2 0.6 does not wipe its own).
struct BlockMemory(Vec<Block>);

impl Drop for BlockMemory {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Allocates `blocks` Argon2 blocks without aborting on failure.
fn alloc_blocks(blocks: usize, needed_mib: u32) -> Result<BlockMemory, VaultError> {
    let mut memory = BlockMemory(Vec::new());
    memory
        .0
        .try_reserve_exact(blocks)
        .map_err(|_| VaultError::OutOfMemory { needed_mib })?;
    memory.0.resize(blocks, Block::new());
    Ok(memory)
}

/// [`derive_kek`] on already-normalised bytes, refusing to use more than `limit_bytes` of
/// working memory (see [`memory_limit`]).
pub(crate) fn derive_kek_limited(
    password: &[u8],
    salt: &[u8; 16],
    params: KdfParams,
    limit_bytes: u64,
) -> Result<SecretBox<[u8; 32]>, VaultError> {
    let params = params.validate_for_read()?;
    let needed_mib = params.needed_mib();
    let argon_params = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(32))
        .map_err(|_| VaultError::UnsupportedKdf)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let blocks = argon.params().block_count();
    let bytes = u64::try_from(blocks)
        .ok()
        .and_then(|b| b.checked_mul(Block::SIZE as u64))
        .ok_or(VaultError::OutOfMemory { needed_mib })?;
    if bytes > limit_bytes {
        return Err(VaultError::OutOfMemory { needed_mib });
    }
    let mut memory = alloc_blocks(blocks, needed_mib)?;
    let mut out = SecretBox::new(Box::new([0u8; 32]));
    argon
        .hash_password_into_with_memory(
            password,
            salt,
            out.expose_secret_mut(),
            memory.0.as_mut_slice(),
        )
        .map_err(|e| match e {
            argon2::Error::OutOfMemory => VaultError::OutOfMemory { needed_mib },
            _ => VaultError::UnsupportedKdf,
        })?;
    drop(memory);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impossible_allocation_is_an_error_not_an_abort() {
        // More blocks than the address space can hold: try_reserve fails without touching
        // memory, which is the path a real out-of-memory takes on Windows (no overcommit).
        let result = alloc_blocks(usize::MAX / Block::SIZE, 7);
        assert!(matches!(
            result,
            Err(VaultError::OutOfMemory { needed_mib: 7 })
        ));
    }

    #[test]
    fn create_floor_is_the_owasp_minimum() {
        assert!(KdfParams::MINIMUM.validate_for_create().is_ok());
        assert!(KdfParams::STANDARD.validate_for_create().is_ok());
        let weak = KdfParams {
            m_cost_kib: 8,
            t_cost: 1,
            p_cost: 1,
        };
        assert!(weak.validate_for_read().is_ok());
        assert!(matches!(
            weak.validate_for_create(),
            Err(VaultError::UnsupportedKdf)
        ));
    }

    #[test]
    fn upgrade_never_lowers_a_parameter() {
        let stored = KdfParams {
            m_cost_kib: 262_144,
            t_cost: 1,
            p_cost: 1,
        };
        assert!(stored.is_weaker_than(KdfParams::STANDARD));
        let next = stored.upgraded_to(KdfParams::STANDARD);
        assert_eq!(
            next,
            KdfParams {
                m_cost_kib: 262_144,
                t_cost: 3,
                p_cost: 1
            }
        );
        assert!(!next.is_weaker_than(KdfParams::STANDARD));
    }

    #[test]
    fn work_cap_bounds_memory_times_passes() {
        let at_cap = KdfParams {
            m_cost_kib: 1_048_576,
            t_cost: 4,
            p_cost: 1,
        };
        assert!(at_cap.validate_for_read().is_ok());
        assert!(at_cap.validate_for_create().is_ok());
        let over = KdfParams {
            t_cost: 5,
            ..at_cap
        };
        assert!(matches!(
            over.validate_for_read(),
            Err(VaultError::UnsupportedKdf)
        ));
        assert!(matches!(
            over.validate_for_create(),
            Err(VaultError::UnsupportedKdf)
        ));
    }

    #[test]
    fn memory_limit_defaults_to_the_largest_readable_header() {
        assert_eq!(memory_limit(None), DEFAULT_MEMORY_LIMIT_BYTES);
        assert_eq!(memory_limit(Some(u64::MAX)), DEFAULT_MEMORY_LIMIT_BYTES);
        assert_eq!(memory_limit(Some(4096)), 4096);
        assert_eq!(DEFAULT_MEMORY_LIMIT_BYTES, 1 << 30);
    }

    #[test]
    fn normalisation_counts_bytes_after_nfkc() {
        let nfd = SecretString::from("cafe\u{301}!!!".to_owned());
        let nfc = SecretString::from("caf\u{e9}!!!".to_owned());
        assert_eq!(normalize_password(&nfd).ok(), normalize_password(&nfc).ok());
        // "ﬃ" (one char) normalises to "ffi": 3 bytes.
        let short = SecretString::from("\u{fb03}\u{fb03}".to_owned());
        assert_eq!(normalize_password(&short).map(|s| s.len()).ok(), None);
    }
}

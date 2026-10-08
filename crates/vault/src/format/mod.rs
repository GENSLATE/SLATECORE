//! Byte-level on-disk formats (research/vault.md 4). Pure (de)serialisation, no I/O.
//!
//! All integers are little-endian. These types are public so the golden vectors can be checked
//! from outside the crate and so external tooling can inspect a vault folder; applications use
//! [`crate::Vault`].

pub mod blob;
pub mod header;
pub mod index;

/// Format version written by this crate for the header, index and blobs.
pub const FORMAT_VERSION: u16 = 1;

/// `bytes[at..at + N]` as an array; callers check the total length first.
pub(crate) fn array<const N: usize>(bytes: &[u8], at: usize) -> [u8; N] {
    let mut out = [0u8; N];
    if let Some(src) = bytes.get(at..at + N) {
        out.copy_from_slice(src);
    }
    out
}

pub(crate) fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(array(bytes, at))
}

pub(crate) fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(array(bytes, at))
}

pub(crate) fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(array(bytes, at))
}

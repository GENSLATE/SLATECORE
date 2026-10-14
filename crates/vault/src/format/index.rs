//! Index slot `index.{a,b}.gvi` and its plaintext (research/vault.md 4.4).
//!
//! | Offset | Len | Field |
//! |---|---|---|
//! | 0 | 8 | magic `GSVINDX\0` |
//! | 8 | 2 | format version (1) |
//! | 10 | 2 | flags (0) |
//! | 12 | 8 | generation (+1 on every commit, starts at 1) |
//! | 20 | 24 | nonce |
//! | 44 | .. | XChaCha20-Poly1305 ciphertext and tag under the vault key |
//!
//! Associated data: `"GSV1-INDEX" || vault_id || bytes[0, 20)`. The plaintext is JSON with the
//! entries sorted by path; names and folders exist nowhere else.

// cspell:ignore gsvindx

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use super::blob::TAG_LEN;
use super::{FORMAT_VERSION, array, u16_at, u64_at};
use crate::EntryKind;
use crate::error::{VaultError, tamper};
use crate::path::VaultPath;

/// Size of the slot header before the ciphertext.
pub const INDEX_HEADER_LEN: usize = 44;
/// `GSVINDX\0`.
pub const MAGIC: [u8; 8] = *b"GSVINDX\0";
/// Largest accepted index plaintext.
pub const MAX_INDEX_PLAINTEXT: usize = 16 * 1024 * 1024;
/// Most entries (files and folders) one vault may hold.
pub const MAX_ENTRIES: usize = 8192;

const INDEX_LABEL: &[u8] = b"GSV1-INDEX";
const SCHEMA: u32 = 1;

/// The unencrypted 44-byte prefix of an index slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexSlotHeader {
    pub generation: u64,
    pub nonce: [u8; 24],
}

impl IndexSlotHeader {
    /// Serialises the prefix.
    pub fn encode(&self) -> [u8; INDEX_HEADER_LEN] {
        let mut b = [0u8; INDEX_HEADER_LEN];
        b[0..8].copy_from_slice(&MAGIC);
        b[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        b[12..20].copy_from_slice(&self.generation.to_le_bytes());
        b[20..44].copy_from_slice(&self.nonce);
        b
    }

    /// Parses the prefix of a whole slot file (which must also hold at least a tag).
    pub fn decode(bytes: &[u8]) -> Result<Self, VaultError> {
        if bytes.len() < INDEX_HEADER_LEN + TAG_LEN || bytes[0..8] != MAGIC {
            return Err(VaultError::Tampered(tamper::INDEX));
        }
        let version = u16_at(bytes, 8);
        if version != FORMAT_VERSION || u16_at(bytes, 10) != 0 {
            return Err(VaultError::UnsupportedVersion(version));
        }
        Ok(Self {
            generation: u64_at(bytes, 12),
            nonce: array(bytes, 20),
        })
    }

    /// Associated data: `"GSV1-INDEX" || vault_id || bytes[0, 20)`.
    pub fn aad(&self, vault_id: &[u8; 16]) -> Vec<u8> {
        let encoded = self.encode();
        let mut aad = Vec::with_capacity(INDEX_LABEL.len() + 16 + 20);
        aad.extend_from_slice(INDEX_LABEL);
        aad.extend_from_slice(vault_id);
        aad.extend_from_slice(&encoded[..20]);
        aad
    }
}

/// One entry of the in-memory index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) kind: EntryKind,
    pub(crate) blob: Option<[u8; 16]>,
    pub(crate) size: u64,
    pub(crate) modified_ms: u64,
}

impl Entry {
    pub(crate) fn dir(modified_ms: u64) -> Self {
        Self {
            kind: EntryKind::Dir,
            blob: None,
            size: 0,
            modified_ms,
        }
    }

    pub(crate) fn file(blob: [u8; 16], size: u64, modified_ms: u64) -> Self {
        Self {
            kind: EntryKind::File,
            blob: Some(blob),
            size,
            modified_ms,
        }
    }
}

/// The decrypted index: logical path to entry. Names are wiped from memory on drop.
#[derive(Clone, Default)]
pub(crate) struct Index {
    pub(crate) entries: BTreeMap<VaultPath, Entry>,
}

impl Drop for Index {
    fn drop(&mut self) {
        for (mut path, _) in std::mem::take(&mut self.entries) {
            path.wipe();
        }
    }
}

#[derive(Serialize, Deserialize)]
struct IndexDoc {
    schema: u32,
    entries: Vec<EntryDoc>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum KindDoc {
    Dir,
    File,
}

#[derive(Serialize, Deserialize)]
struct EntryDoc {
    path: String,
    kind: KindDoc,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blob: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    modified_ms: u64,
}

impl Drop for EntryDoc {
    fn drop(&mut self) {
        self.path.zeroize();
    }
}

impl Index {
    /// Serialises to the JSON plaintext into `out` (a buffer that wipes what it frees).
    pub(crate) fn to_json(&self, out: &mut impl std::io::Write) -> Result<(), VaultError> {
        let doc = IndexDoc {
            schema: SCHEMA,
            entries: self
                .entries
                .iter()
                .map(|(path, e)| EntryDoc {
                    path: path.as_str().to_owned(),
                    kind: match e.kind {
                        EntryKind::Dir => KindDoc::Dir,
                        EntryKind::File => KindDoc::File,
                    },
                    blob: e.blob.map(|id| blob_name(&id)),
                    size: (e.kind == EntryKind::File).then_some(e.size),
                    modified_ms: e.modified_ms,
                })
                .collect(),
        };
        serde_json::to_writer(out, &doc).map_err(|_| VaultError::Internal("index serialisation"))
    }

    /// Parses the JSON plaintext and checks every invariant (unique paths, case-insensitively
    /// unique names per folder, parents exist, unique blob ids, entry cap). Any violation is
    /// [`VaultError::Tampered`]`("index")`.
    pub(crate) fn from_json(json: &[u8]) -> Result<Self, VaultError> {
        let bad = || VaultError::Tampered(tamper::INDEX);
        let doc: IndexDoc = serde_json::from_slice(json).map_err(|_| bad())?;
        if doc.schema != SCHEMA || doc.entries.len() > MAX_ENTRIES {
            return Err(bad());
        }
        let mut index = Index::default();
        let mut folded = HashSet::new();
        let mut blobs = HashSet::new();
        for e in &doc.entries {
            let path = VaultPath::parse(&e.path).map_err(|_| bad())?;
            if path.is_root() || !folded.insert(path.fold()) {
                return Err(bad());
            }
            let entry = match (&e.kind, &e.blob, e.size) {
                (KindDoc::Dir, None, None) => Entry::dir(e.modified_ms),
                (KindDoc::File, Some(hex), Some(size)) => {
                    let id = parse_blob_name(hex).ok_or_else(bad)?;
                    if !blobs.insert(id) {
                        return Err(bad());
                    }
                    Entry::file(id, size, e.modified_ms)
                }
                _ => return Err(bad()),
            };
            if index.entries.insert(path, entry).is_some() {
                return Err(bad());
            }
        }
        for path in index.entries.keys() {
            let parent = path.parent().ok_or_else(bad)?;
            if !parent.is_root()
                && index.entries.get(&parent).map(|p| p.kind) != Some(EntryKind::Dir)
            {
                return Err(bad());
            }
        }
        Ok(index)
    }
}

/// Lowercase hex of a blob id: the file stem under `files/`.
pub(crate) fn blob_name(id: &[u8; 16]) -> String {
    use std::fmt::Write as _;
    id.iter().fold(String::with_capacity(32), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Parses 32 lowercase hex digits.
pub(crate) fn parse_blob_name(s: &str) -> Option<[u8; 16]> {
    let bytes = s.as_bytes();
    if bytes.len() != 32 {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; 16];
    let (pairs, _) = bytes.as_chunks::<2>();
    for (i, [high, low]) in pairs.iter().enumerate() {
        out[i] = (digit(*high)? << 4) | digit(*low)?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(s: &str) -> Result<Index, VaultError> {
        Index::from_json(s.as_bytes())
    }

    #[test]
    fn invariants_are_checked() {
        let id_a = "0".repeat(32);
        let id_b = "1".repeat(32);
        let ok = format!(
            r#"{{"schema":1,"entries":[{{"path":"T","kind":"dir","modified_ms":1}},
            {{"path":"T/a","kind":"file","blob":"{id_a}","size":3,"modified_ms":1}}]}}"#
        );
        assert!(json(&ok).is_ok());
        let cases = [
            // parent missing
            format!(
                r#"{{"schema":1,"entries":[{{"path":"T/a","kind":"file","blob":"{id_a}","size":3,"modified_ms":1}}]}}"#
            ),
            // duplicate blob id
            format!(
                r#"{{"schema":1,"entries":[{{"path":"a","kind":"file","blob":"{id_a}","size":3,"modified_ms":1}},{{"path":"b","kind":"file","blob":"{id_a}","size":3,"modified_ms":1}}]}}"#
            ),
            // case-insensitive duplicate
            format!(
                r#"{{"schema":1,"entries":[{{"path":"a","kind":"file","blob":"{id_a}","size":3,"modified_ms":1}},{{"path":"A","kind":"file","blob":"{id_b}","size":3,"modified_ms":1}}]}}"#
            ),
            // unknown kind, invalid name, uppercase hex, file without blob, schema 2
            r#"{"schema":1,"entries":[{"path":"a","kind":"link","modified_ms":1}]}"#.to_owned(),
            r#"{"schema":1,"entries":[{"path":"CON","kind":"dir","modified_ms":1}]}"#.to_owned(),
            format!(
                r#"{{"schema":1,"entries":[{{"path":"a","kind":"file","blob":"{}","size":3,"modified_ms":1}}]}}"#,
                "A".repeat(32)
            ),
            r#"{"schema":1,"entries":[{"path":"a","kind":"file","modified_ms":1}]}"#.to_owned(),
            r#"{"schema":2,"entries":[]}"#.to_owned(),
        ];
        for case in &cases {
            assert!(
                matches!(json(case), Err(VaultError::Tampered("index"))),
                "{case}"
            );
        }
    }

    #[test]
    fn json_round_trip() -> Result<(), VaultError> {
        let mut index = Index::default();
        index
            .entries
            .insert(VaultPath::parse("Docs")?, Entry::dir(5));
        index.entries.insert(
            VaultPath::parse("Docs/a.txt")?,
            Entry::file([0xab; 16], 9, 6),
        );
        let mut out = Vec::new();
        index.to_json(&mut out)?;
        let back = Index::from_json(&out)?;
        assert_eq!(back.entries, index.entries);
        Ok(())
    }
}

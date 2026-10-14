//! Logical vault paths (research/vault.md 4.5).
//!
//! A [`VaultPath`] is a validated, `/`-separated, relative path inside the vault; `""` is the
//! root. Every name is checked when it is created so that any entry can later be exported to a
//! Windows file system: components of 1 to 255 UTF-16 units, at most 16 levels, at most 1024
//! UTF-16 units in total, no reserved characters or device names, no trailing space or period.

// cspell:ignore conin conout rfind rsplit

use zeroize::Zeroize;

use crate::error::VaultError;

/// Longest component, in UTF-16 code units (NTFS, exFAT and FAT32 all allow 255).
pub(crate) const MAX_COMPONENT_UNITS: usize = 255;
/// Deepest path, in components.
pub(crate) const MAX_DEPTH: usize = 16;
/// Longest path, in UTF-16 code units including the separators.
pub(crate) const MAX_PATH_UNITS: usize = 1024;

const RESERVED_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
/// Device names Windows reserves, with or without an extension (Microsoft "Naming Files, Paths,
/// and Namespaces": `COM0` and `LPT0` included, plus the console names `CONIN$` and `CONOUT$`).
const RESERVED_NAMES: &[&str] = &[
    "con",
    "prn",
    "aux",
    "nul",
    "conin$",
    "conout$",
    "com0",
    "com1",
    "com2",
    "com3",
    "com4",
    "com5",
    "com6",
    "com7",
    "com8",
    "com9",
    "com\u{b9}",
    "com\u{b2}",
    "com\u{b3}",
    "lpt0",
    "lpt1",
    "lpt2",
    "lpt3",
    "lpt4",
    "lpt5",
    "lpt6",
    "lpt7",
    "lpt8",
    "lpt9",
    "lpt\u{b9}",
    "lpt\u{b2}",
    "lpt\u{b3}",
];

/// Validated, "/"-separated, relative logical path. "" is the vault root.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VaultPath(String);

impl VaultPath {
    /// The vault root, `""`.
    pub fn root() -> Self {
        Self(String::new())
    }

    /// Validates `s` as a full logical path. `""` is the root.
    pub fn parse(s: &str) -> Result<Self, VaultError> {
        if s.is_empty() {
            return Ok(Self::root());
        }
        let mut depth = 0;
        for component in s.split('/') {
            validate_component(component)?;
            depth += 1;
        }
        if depth > MAX_DEPTH {
            return Err(VaultError::InvalidPath("folders are nested too deeply"));
        }
        if s.encode_utf16().count() > MAX_PATH_UNITS {
            return Err(VaultError::InvalidPath("path is too long"));
        }
        Ok(Self(s.to_owned()))
    }

    /// The path as stored in the index.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The containing folder; `None` for the root.
    pub fn parent(&self) -> Option<VaultPath> {
        if self.0.is_empty() {
            return None;
        }
        Some(match self.0.rfind('/') {
            Some(i) => Self(self.0[..i].to_owned()),
            None => Self::root(),
        })
    }

    /// The last component; `None` for the root.
    pub fn file_name(&self) -> Option<&str> {
        if self.0.is_empty() {
            return None;
        }
        Some(self.0.rsplit('/').next().unwrap_or(&self.0))
    }

    /// Appends one component (`name` must not contain `/`).
    pub fn join(&self, name: &str) -> Result<VaultPath, VaultError> {
        validate_component(name)?;
        if self.0.is_empty() {
            Self::parse(name)
        } else {
            Self::parse(&format!("{}/{name}", self.0))
        }
    }

    /// `true` for the root path.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// `true` when `self` is `other` or lies inside it.
    pub fn starts_with(&self, other: &VaultPath) -> bool {
        other.0.is_empty()
            || self.0 == other.0
            || (self.0.starts_with(&other.0) && self.0.as_bytes().get(other.0.len()) == Some(&b'/'))
    }

    /// Number of components (0 for the root).
    pub(crate) fn depth(&self) -> usize {
        if self.0.is_empty() {
            0
        } else {
            self.0.split('/').count()
        }
    }

    /// Replaces the leading `from` with `to` (used when a folder is renamed).
    pub(crate) fn rebase(&self, from: &VaultPath, to: &VaultPath) -> Result<VaultPath, VaultError> {
        if self == from {
            return Ok(to.clone());
        }
        let rest = self
            .0
            .get(from.0.len() + 1..)
            .ok_or(VaultError::Internal("rebase"))?;
        if to.0.is_empty() {
            Self::parse(rest)
        } else {
            Self::parse(&format!("{}/{rest}", to.0))
        }
    }

    /// Overwrites the name in memory (used when the index is dropped).
    pub(crate) fn wipe(&mut self) {
        self.0.zeroize();
    }

    /// The case-insensitive comparison key used for name conflicts.
    pub(crate) fn fold(&self) -> String {
        self.0.to_lowercase()
    }
}

/// Checks one path component against the Windows-exportable naming rules.
pub(crate) fn validate_component(name: &str) -> Result<(), VaultError> {
    if name.is_empty() {
        return Err(VaultError::InvalidPath("empty name"));
    }
    if name == "." || name == ".." {
        return Err(VaultError::InvalidPath("'.' and '..' are not names"));
    }
    if name.encode_utf16().count() > MAX_COMPONENT_UNITS {
        return Err(VaultError::InvalidPath(
            "name is longer than 255 characters",
        ));
    }
    if name.chars().any(|c| c < ' ' || RESERVED_CHARS.contains(&c)) {
        return Err(VaultError::InvalidPath(
            "name contains a reserved character",
        ));
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Err(VaultError::InvalidPath(
            "name ends with a space or a period",
        ));
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end_matches(' ')
        .to_lowercase();
    if RESERVED_NAMES.contains(&stem.as_str()) {
        return Err(VaultError::InvalidPath("name is reserved by Windows"));
    }
    Ok(())
}

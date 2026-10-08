//! Helpers shared by the vault integration tests.
//!
//! Every test returns [`TestResult`] and uses `?`: the workspace denies `unwrap`/`expect`.

// cspell:ignore hexdigit mklink
#![allow(dead_code)] // each test file uses a different subset

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use genslate_vault::format::index::IndexSlotHeader;
use genslate_vault::{KdfParams, SecretString, Vault, VaultConfig, VaultError, VaultPath};

pub type TestResult = Result<(), Box<dyn Error>>;

pub const PASSWORD: &str = "correct horse battery staple";
pub const OTHER_PASSWORD: &str = "a different passphrase";

/// A temporary installDir-like folder with a vault root and a session root.
pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub config: VaultConfig,
}

impl Fixture {
    /// Vault at `<tmp>/storage/vault`, session at `<tmp>/other/launcher/cache/vault-session`,
    /// KDF at the production floor ([`KdfParams::MINIMUM`]) so tests stay fast.
    pub fn new() -> io::Result<Self> {
        let dir = tempfile::tempdir()?;
        let config = VaultConfig {
            root: dir.path().join("storage").join("vault"),
            session_root: dir
                .path()
                .join("other")
                .join("launcher")
                .join("cache")
                .join("vault-session"),
            kdf: KdfParams::MINIMUM,
            max_blob_bytes: None,
        };
        Ok(Self { dir, config })
    }

    pub fn root(&self) -> &Path {
        &self.config.root
    }

    pub fn session_root(&self) -> &Path {
        &self.config.session_root
    }

    pub fn files_dir(&self) -> PathBuf {
        self.config.root.join("files")
    }

    /// A scratch folder outside the vault for source and export files.
    pub fn scratch(&self) -> io::Result<PathBuf> {
        let path = self.dir.path().join("scratch");
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn open(&self) -> Result<Vault, VaultError> {
        Ok(Vault::open(self.config.clone())?.0)
    }

    /// Opens the vault and creates it with [`PASSWORD`]; it is left unlocked.
    pub fn create(&self) -> Result<Vault, VaultError> {
        let vault = self.open()?;
        vault.create(&pw(PASSWORD))?;
        Ok(vault)
    }

    /// Writes `bytes` to `scratch/<name>` and imports it into `dir`.
    pub fn import_bytes(
        &self,
        vault: &Vault,
        dir: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        let src_dir = self.scratch()?.join(format!("src-{}", unique()));
        fs::create_dir_all(&src_dir)?;
        let src = src_dir.join(name);
        fs::write(&src, bytes)?;
        vault.import(
            &src,
            &VaultPath::parse(dir)?,
            genslate_vault::Conflict::Fail,
            None,
        )?;
        fs::remove_dir_all(&src_dir)?;
        Ok(())
    }

    /// Exports `path` to a fresh scratch file and returns its bytes.
    pub fn export_bytes(&self, vault: &Vault, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        let dest = self.scratch()?.join(format!("export-{}", unique()));
        vault.export(&VaultPath::parse(path)?, &dest, None)?;
        let bytes = fs::read(&dest)?;
        fs::remove_file(&dest)?;
        Ok(bytes)
    }

    /// Every `files/*.gvf` blob, sorted.
    pub fn blobs(&self) -> io::Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        if self.files_dir().is_dir() {
            for entry in fs::read_dir(self.files_dir())? {
                let path = entry?.path();
                if path.extension().is_some_and(|ext| ext == "gvf") {
                    out.push(path);
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

pub fn pw(s: &str) -> SecretString {
    SecretString::from(s.to_owned())
}

/// Process-unique counter for scratch names.
pub fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Deterministic pseudo-random bytes (xor-shift, 64-bit), so tests need no `rand`.
pub fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let word = state.wrapping_mul(0x2545_F491_4F6C_DD1D).to_le_bytes();
        let take = (len - out.len()).min(8);
        out.extend_from_slice(&word[..take]);
    }
    out
}

/// Bytes of every regular file under `dir`, keyed by path relative to `dir`.
pub fn snapshot(dir: &Path) -> io::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(dir)
                    .map_err(io::Error::other)?
                    .to_path_buf();
                out.insert(rel, fs::read(&path)?);
            }
        }
    }
    Ok(out)
}

/// Number of regular files under `dir` (0 when it does not exist).
pub fn count_files(dir: &Path) -> io::Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    Ok(snapshot(dir)?.len())
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

pub fn parse_hex(s: &str) -> Vec<u8> {
    let digits: Vec<u8> = s
        .bytes()
        .filter(u8::is_ascii_hexdigit)
        .map(|c| match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => c - b'A' + 10,
        })
        .collect();
    digits
        .chunks(2)
        .map(|pair| (pair[0] << 4) | pair[1])
        .collect()
}

/// `true` when `haystack` contains `needle` as a byte substring.
pub fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// Names of the entries listed in `dir`, sorted.
pub fn names(vault: &Vault, dir: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut out: Vec<String> = vault
        .list(&VaultPath::parse(dir)?)?
        .into_iter()
        .filter_map(|e| e.path.file_name().map(str::to_owned))
        .collect();
    out.sort();
    Ok(out)
}

/// Asserts that `items` is empty, printing them otherwise.
#[track_caller]
pub fn assert_empty<T: std::fmt::Debug>(items: &[T]) {
    assert!(items.is_empty(), "expected nothing, got {items:?}");
}

/// The generation stored in an index slot file.
pub fn index_generation(path: &Path) -> Result<u64, Box<dyn Error>> {
    Ok(IndexSlotHeader::decode(&fs::read(path)?)?.generation)
}

/// Creates a symbolic link to a file. `Ok(false)` when Windows refuses it for lack of the
/// privilege, so the caller can skip that part of a test.
pub fn symlink_file(target: &Path, link: &Path) -> io::Result<bool> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
        Ok(true)
    }
    #[cfg(windows)]
    {
        match std::os::windows::fs::symlink_file(target, link) {
            Ok(()) => Ok(true),
            Err(e) if e.raw_os_error() == Some(1314) => Ok(false), // ERROR_PRIVILEGE_NOT_HELD
            Err(e) => Err(e),
        }
    }
}

/// Creates a link to a folder: a symbolic link on Unix, a junction on Windows (no privilege
/// needed). `Ok(false)` when it could not be made.
pub fn symlink_dir(target: &Path, link: &Path) -> io::Result<bool> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
        Ok(true)
    }
    #[cfg(windows)]
    {
        let status = std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .stdout(std::process::Stdio::null())
            .status()?;
        Ok(status.success())
    }
}

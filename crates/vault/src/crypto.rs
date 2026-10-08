//! Key wrapping, index sealing and streaming blob encryption (research/vault.md 3, 4.3, 4.4).
//!
//! Everything is XChaCha20-Poly1305. Large files use the STREAM construction from
//! `aead-stream` (`StreamBE32`): a 19-byte random prefix, a 32-bit big-endian chunk counter and
//! a last-chunk flag, so reordered, dropped, appended or truncated chunks fail authentication.
//! Plaintext buffers are wiped after use.

use std::io::{self, Read, Write};

use aead_stream::aead::array::Array;
use aead_stream::{DecryptorBE32, EncryptorBE32};
use blake2::digest::consts::U32;
use blake2::{Blake2b, Digest};
use chacha20poly1305::{AeadInOut, KeyInit, XChaCha20Poly1305, XNonce};
use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox};
use zeroize::{Zeroize, Zeroizing};

use crate::error::{VaultError, tamper};
use crate::format::blob::{BLOB_HEADER_LEN, BlobHeader, BlobLayout, TAG_LEN};
use crate::format::index::{INDEX_HEADER_LEN, Index, IndexSlotHeader, MAX_INDEX_PLAINTEXT};

/// A 32-byte secret key that is wiped on drop and never printed by `Debug`.
pub type SecretKey = SecretBox<[u8; 32]>;

/// Bytes the FAT32 4 GiB limit allows before a full disk is treated as "file too large".
const FAT32_NEAR_LIMIT: u64 = 0xFFFF_0000;

/// Fills `N` bytes from the operating system's CSPRNG.
pub(crate) fn random<const N: usize>() -> Result<[u8; N], VaultError> {
    let mut out = [0u8; N];
    getrandom::fill(&mut out)
        .map_err(|_| VaultError::Internal("the system random generator failed"))?;
    Ok(out)
}

/// A fresh random key, generated in place on the heap.
pub(crate) fn random_key() -> Result<SecretKey, VaultError> {
    let mut key = SecretBox::new(Box::new([0u8; 32]));
    getrandom::fill(key.expose_secret_mut())
        .map_err(|_| VaultError::Internal("the system random generator failed"))?;
    Ok(key)
}

/// A cipher keyed straight from the secret (no stack copy of the key bytes).
pub(crate) fn cipher(key: &SecretKey) -> Result<XChaCha20Poly1305, VaultError> {
    XChaCha20Poly1305::new_from_slice(key.expose_secret())
        .map_err(|_| VaultError::Internal("key length"))
}

/// Wraps the vault key under the key-encryption key: 32 bytes of ciphertext and a 16-byte tag.
pub fn wrap_vault_key(
    kek: &SecretKey,
    nonce: &[u8; 24],
    vault_key: &SecretKey,
    aad: &[u8],
) -> Result<[u8; 48], VaultError> {
    let mut buf = Zeroizing::new(Vec::with_capacity(48));
    buf.extend_from_slice(vault_key.expose_secret());
    cipher(kek)?
        .encrypt_in_place(&XNonce::from(*nonce), aad, &mut *buf)
        .map_err(|_| VaultError::Internal("key wrap"))?;
    let mut out = [0u8; 48];
    if buf.len() != out.len() {
        return Err(VaultError::Internal("key wrap length"));
    }
    out.copy_from_slice(&buf);
    Ok(out)
}

/// Unwraps the vault key. An authentication failure is [`VaultError::WrongPassword`]: the wrap
/// doubles as the password check.
pub fn unwrap_vault_key(
    kek: &SecretKey,
    nonce: &[u8; 24],
    wrapped: &[u8; 48],
    aad: &[u8],
) -> Result<SecretKey, VaultError> {
    let mut buf = Zeroizing::new(wrapped.to_vec());
    cipher(kek)?
        .decrypt_in_place(&XNonce::from(*nonce), aad, &mut *buf)
        .map_err(|_| VaultError::WrongPassword)?;
    let mut key = SecretBox::new(Box::new([0u8; 32]));
    if buf.len() != 32 {
        return Err(VaultError::Internal("key unwrap length"));
    }
    key.expose_secret_mut().copy_from_slice(&buf);
    Ok(key)
}

/// A growable byte buffer that wipes every allocation it gives up (used for plaintext JSON).
pub(crate) struct SecretBuf {
    buf: Zeroizing<Vec<u8>>,
    max: usize,
}

impl SecretBuf {
    pub(crate) fn new(capacity: usize, max: usize) -> Self {
        Self {
            buf: Zeroizing::new(Vec::with_capacity(capacity)),
            max,
        }
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.buf
    }
}

impl Write for SecretBuf {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let needed = self.buf.len() + data.len();
        if needed > self.max {
            return Err(io::Error::other("buffer limit"));
        }
        if needed > self.buf.capacity() {
            let mut grown = Zeroizing::new(Vec::with_capacity(needed.max(self.buf.capacity() * 2)));
            grown.extend_from_slice(&self.buf);
            self.buf = grown; // the old allocation is wiped on drop
        }
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Encrypts the index into a complete slot file of the given generation.
pub(crate) fn seal_index(
    key: &SecretKey,
    vault_id: &[u8; 16],
    generation: u64,
    index: &Index,
) -> Result<Vec<u8>, VaultError> {
    let mut json = SecretBuf::new(16 * 1024, MAX_INDEX_PLAINTEXT);
    index
        .to_json(&mut json)
        .map_err(|_| VaultError::LimitExceeded("index size"))?;
    let header = IndexSlotHeader {
        generation,
        nonce: random()?,
    };
    let mut sealed = Zeroizing::new(Vec::with_capacity(json.as_slice().len() + TAG_LEN));
    sealed.extend_from_slice(json.as_slice());
    cipher(key)?
        .encrypt_in_place(
            &XNonce::from(header.nonce),
            &header.aad(vault_id),
            &mut *sealed,
        )
        .map_err(|_| VaultError::Internal("index encryption"))?;
    let mut out = Vec::with_capacity(INDEX_HEADER_LEN + sealed.len());
    out.extend_from_slice(&header.encode());
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Decrypts and validates one index slot file.
pub(crate) fn open_index(
    key: &SecretKey,
    vault_id: &[u8; 16],
    bytes: &[u8],
) -> Result<(u64, Index), VaultError> {
    let header = IndexSlotHeader::decode(bytes)?;
    let body = bytes
        .get(INDEX_HEADER_LEN..)
        .ok_or(VaultError::Tampered(tamper::INDEX))?;
    if body.len() > MAX_INDEX_PLAINTEXT + TAG_LEN {
        return Err(VaultError::Tampered(tamper::INDEX));
    }
    let mut plain = Zeroizing::new(body.to_vec());
    cipher(key)?
        .decrypt_in_place(
            &XNonce::from(header.nonce),
            &header.aad(vault_id),
            &mut *plain,
        )
        .map_err(|_| VaultError::Tampered(tamper::INDEX))?;
    Ok((header.generation, Index::from_json(&plain)?))
}

/// What a blob stream produced: plaintext length and its `BLAKE2b-256` (change detection).
#[derive(Clone, Copy, Debug)]
pub(crate) struct StreamSummary {
    pub(crate) plain_len: u64,
    pub(crate) hash: [u8; 32],
}

/// Called after every chunk with the plaintext bytes processed so far; an `Err` aborts the
/// stream (the vault uses it for progress and to cancel when the vault is locked).
pub(crate) type ChunkHook<'a> = &'a mut dyn FnMut(u64) -> Result<(), VaultError>;

/// Reads until `buf` is full or the reader is exhausted; returns the bytes read.
fn read_full(reader: &mut dyn Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// Fills `buf` with the next chunk of up to `chunk` bytes, reusing its allocation.
fn next_chunk(reader: &mut dyn Read, buf: &mut Vec<u8>, chunk: usize) -> Result<(), VaultError> {
    buf.clear();
    buf.resize(chunk, 0);
    let n = read_full(reader, buf).map_err(VaultError::io("reading a file to encrypt"))?;
    buf.truncate(n);
    Ok(())
}

/// Maps a write error, recognising the FAT32 file-size limit.
fn write_error(e: io::Error, written: u64) -> VaultError {
    match e.kind() {
        io::ErrorKind::FileTooLarge => VaultError::FileTooLarge,
        io::ErrorKind::StorageFull if written >= FAT32_NEAR_LIMIT => VaultError::FileTooLarge,
        _ => VaultError::Io {
            context: "writing an encrypted file",
            source: e,
        },
    }
}

/// Encrypts `reader` into `writer` as a complete blob (header and chunks).
pub(crate) fn encrypt_stream(
    key: &SecretKey,
    vault_id: &[u8; 16],
    header: &BlobHeader,
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    max_blob_bytes: Option<u64>,
    hook: ChunkHook<'_>,
) -> Result<StreamSummary, VaultError> {
    let chunk =
        usize::try_from(header.chunk_size).map_err(|_| VaultError::Internal("chunk size"))?;
    let aad = header.aad(vault_id);
    let prefix = Array::from(header.stream_nonce);
    let mut encryptor = Some(EncryptorBE32::<XChaCha20Poly1305>::from_aead(
        cipher(key)?,
        &prefix,
    ));
    let mut hasher = Blake2b::<U32>::new();
    let mut current = Zeroizing::new(Vec::with_capacity(chunk + TAG_LEN));
    let mut next = Zeroizing::new(Vec::with_capacity(chunk + TAG_LEN));
    let mut written = BLOB_HEADER_LEN as u64;
    let mut plain_len = 0u64;
    writer
        .write_all(&header.encode())
        .map_err(|e| write_error(e, 0))?;
    next_chunk(reader, &mut current, chunk)?;
    loop {
        next_chunk(reader, &mut next, chunk)?;
        let last = next.is_empty();
        hasher.update(current.as_slice());
        plain_len += current.len() as u64;
        let sealed_len = (current.len() + TAG_LEN) as u64;
        if max_blob_bytes.is_some_and(|max| written + sealed_len > max) {
            return Err(VaultError::FileTooLarge);
        }
        let mut stream = encryptor
            .take()
            .ok_or(VaultError::Internal("stream state"))?;
        let sealed = if last {
            stream.encrypt_last_in_place(&aad, &mut *current)
        } else {
            let sealed = stream.encrypt_next_in_place(&aad, &mut *current);
            encryptor = Some(stream);
            sealed
        };
        sealed.map_err(|_| VaultError::LimitExceeded("file is too large to encrypt"))?;
        writer
            .write_all(&current)
            .map_err(|e| write_error(e, written))?;
        written += sealed_len;
        hook(plain_len)?;
        if last {
            break;
        }
        std::mem::swap(&mut current, &mut next);
    }
    writer.flush().map_err(|e| write_error(e, written))?;
    Ok(StreamSummary {
        plain_len,
        hash: hasher.finalize().into(),
    })
}

/// Decrypts a blob of `file_len` bytes from `reader` into `writer`.
///
/// Checks, in order: header fields, `expected_id` (the file name and index entry), the length
/// structure, `expected_size` (the index), then every chunk's tag. Plaintext reaches `writer`
/// chunk by chunk, so callers must discard the output unless this returns `Ok` (the final chunk
/// is only authenticated at the end).
#[allow(clippy::too_many_arguments)] // one call site per transfer kind; a struct adds nothing
pub(crate) fn decrypt_stream(
    key: &SecretKey,
    vault_id: &[u8; 16],
    expected_id: &[u8; 16],
    reader: &mut dyn Read,
    file_len: u64,
    expected_size: Option<u64>,
    writer: &mut dyn Write,
    hook: ChunkHook<'_>,
) -> Result<StreamSummary, VaultError> {
    let mut head = [0u8; BLOB_HEADER_LEN];
    if read_full(reader, &mut head).map_err(VaultError::io("reading an encrypted file"))?
        < head.len()
    {
        return Err(VaultError::Tampered(tamper::TRUNCATED));
    }
    let header = BlobHeader::decode(&head)?;
    if header.blob_id != *expected_id {
        return Err(VaultError::Tampered(tamper::BLOB_ID));
    }
    let layout = BlobLayout::from_file_len(file_len, header.chunk_size)?;
    if expected_size.is_some_and(|size| size != layout.plain_len) {
        return Err(VaultError::Tampered(tamper::SIZE));
    }
    let sealed_chunk = u64::from(header.chunk_size) + TAG_LEN as u64;
    let aad = header.aad(vault_id);
    let prefix = Array::from(header.stream_nonce);
    let mut decryptor = Some(DecryptorBE32::<XChaCha20Poly1305>::from_aead(
        cipher(key)?,
        &prefix,
    ));
    let capacity = usize::try_from(sealed_chunk).map_err(|_| VaultError::Internal("chunk size"))?;
    let mut buf = Zeroizing::new(Vec::with_capacity(capacity));
    let mut hasher = Blake2b::<U32>::new();
    let mut done = 0u64;
    let mut remaining = file_len - BLOB_HEADER_LEN as u64;
    for i in 0..layout.chunks {
        let last = i + 1 == layout.chunks;
        let len = if last { remaining } else { sealed_chunk };
        remaining -= len;
        buf.clear();
        buf.resize(
            usize::try_from(len).map_err(|_| VaultError::Internal("chunk size"))?,
            0,
        );
        let got =
            read_full(reader, &mut buf).map_err(VaultError::io("reading an encrypted file"))?;
        if got < buf.len() {
            return Err(VaultError::Tampered(tamper::TRUNCATED));
        }
        let mut stream = decryptor
            .take()
            .ok_or(VaultError::Internal("stream state"))?;
        let opened = if last {
            stream.decrypt_last_in_place(&aad, &mut *buf)
        } else {
            let opened = stream.decrypt_next_in_place(&aad, &mut *buf);
            decryptor = Some(stream);
            opened
        };
        opened.map_err(|_| VaultError::Tampered(tamper::BLOB))?;
        hasher.update(buf.as_slice());
        writer
            .write_all(&buf)
            .map_err(VaultError::io("writing a decrypted file"))?;
        done += buf.len() as u64;
        hook(done)?;
    }
    writer
        .flush()
        .map_err(VaultError::io("writing a decrypted file"))?;
    buf.zeroize();
    Ok(StreamSummary {
        plain_len: done,
        hash: hasher.finalize().into(),
    })
}

/// Encrypts `plaintext` into `out` as a complete blob; returns the plaintext length.
///
/// The public, pure form of the vault's blob writer (no cancellation, no size limit).
pub fn encrypt_blob(
    vault_key: &SecretKey,
    vault_id: &[u8; 16],
    header: &BlobHeader,
    mut plaintext: impl Read,
    mut out: impl Write,
) -> Result<u64, VaultError> {
    let mut hook = |_: u64| Ok(());
    encrypt_stream(
        vault_key,
        vault_id,
        header,
        &mut plaintext,
        &mut out,
        None,
        &mut hook,
    )
    .map(|s| s.plain_len)
}

/// Decrypts a complete blob of `blob_len` bytes into `out`; returns the plaintext length.
///
/// `out` may have received some plaintext when this fails; discard it in that case.
pub fn decrypt_blob(
    vault_key: &SecretKey,
    vault_id: &[u8; 16],
    expected_id: &[u8; 16],
    mut blob: impl Read,
    blob_len: u64,
    mut out: impl Write,
) -> Result<u64, VaultError> {
    let mut hook = |_: u64| Ok(());
    decrypt_stream(
        vault_key,
        vault_id,
        expected_id,
        &mut blob,
        blob_len,
        None,
        &mut out,
        &mut hook,
    )
    .map(|s| s.plain_len)
}

/// `BLAKE2b-256` of everything `reader` yields (session change detection).
pub(crate) fn hash_reader(reader: &mut dyn Read) -> io::Result<[u8; 32]> {
    let mut hasher = Blake2b::<U32>::new();
    let mut buf = Zeroizing::new(vec![0u8; 64 * 1024]);
    loop {
        let n = read_full(reader, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().into())
}

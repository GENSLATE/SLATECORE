---
paths:
  - "crates/vault/**"
  - "**/storage/vault/**"
  - "programs/desktop/*/src-tauri/src/**/vault*"
  - "programs/desktop/*/src/**/vault*"
  - "packages/tauri-bridge/src/**/vault*"
---

# Vault security

Only `storage/vault` is encrypted. These rules are not optional; run the `security-reviewer`
agent on any diff that touches these paths.

- **Crypto:** Argon2id derives the key (parameters are stored in the versioned header).
  XChaCha20-Poly1305 encrypts contents and names, with a fresh random 24-byte nonce per
  encryption, never reused. A random vault key is wrapped by the password key. The header binds
  salt, KDF parameters and a verifier; headers and ids are authenticated as associated data.
- **Secrets in memory only,** zeroised on drop (`zeroize`, `secrecy`). Never log, format, debug
  print or serialise a password or key (`skip(password)` in `tracing`). Compare the verifier in
  constant time (`subtle`). The password crosses IPC once and is never kept in webview state,
  `localStorage` or a file. Password rules: 8 to 1024 bytes after NFKC normalisation.
- **Wrong passwords** are rate limited and never modify files. There is no recovery path; Settings
  says so.
- **Working files:** decrypted copies live only in the session folder inside installDir, are
  re-encrypted on edit, and are removed on lock, exit, hide timeout and at the next start after a
  crash. Wiping is best effort: SSD wear levelling means deletion is not guaranteed, and the UI
  and README say so. Crash safety: two generation slots (A/B) for header and index.
- **IPC takes ids, never paths.** The shell canonicalises and prefix-checks against the vault
  root, rejects `..`, drive-qualified and UNC input, and returns typed errors without leaking paths.
- **Tests required:** round trip, wrong password, tamper detection (flip a bit), header and
  version handling, rate limit, no plaintext left after lock, crash recovery. Fixtures are
  synthetic. Agents never read real vault data (`Read(**/storage/vault/**)` is denied).
- **Dependencies:** RustCrypto crates only, pinned in the workspace; ask before adding any.

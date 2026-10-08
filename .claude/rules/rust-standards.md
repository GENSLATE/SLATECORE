---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
  - "crates/**"
  - "programs/**/src-tauri/**"
---

# Rust and Tauri standards

- **Toolchain:** the exact channel in `rust-toolchain.toml`, edition 2024, target
  `x86_64-pc-windows-msvc` only. No `cfg` branches or dependencies for macOS or Linux.
- **Workspace lints** (root `Cargo.toml`, every crate sets `[lints] workspace = true`):
  `unsafe_code` is denied (one allowed module is documented in the launcher shell, nowhere else),
  clippy `all` and `pedantic` warn, `unwrap_used`, `expect_used` and `dbg_macro` are denied.
  Handle errors; `unwrap` and `expect` are for tests only. No `println!` in libraries.
- **Errors and logs:** `thiserror` enums per crate, `?` to propagate, `tracing` or `log` for
  diagnostics. Never log secrets, passwords, file contents or absolute user paths.
- **Dependencies:** versions live in `[workspace.dependencies]` (caret in the manifest, exact in
  the committed `Cargo.lock`, always `--locked`). Ask before adding a crate. `cargo deny` and
  `cargo machete` run in `bun run check --rust`.
- **Crate shape:** pure logic in the libraries (`crates/paths`, `launcher-core`, `vault`), a thin
  Tauri shell in `programs/desktop/<app>/src-tauri`. Libraries do not depend on Tauri. Crates have
  no `package.json`; Rust tasks run once at the root as `//#rust:*`.
- **Tauri 2:** commands are small, take ids or typed values (never paths from the webview), return
  typed results, and have a matching entry in `capabilities/`. Least privilege: only the
  permissions a window needs. Plugins that touch the OS (dialog, updater, opener defaults,
  deep-link, store, window-state) stay unregistered; see `portability-no-trace.md`. The window
  background colour comes from `genslate-design-tokens`, not a literal.
- **Format and lint:** `rustfmt --edition 2024` (the format-on-edit hook runs it),
  `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- **Tests:** unit tests beside the code, integration tests in `crates/<name>/tests/`, shared
  helpers in `genslate-testing`. See `testing.md`.
- **Windows facts:** paths are case-insensitive, names like `con` and `nul` are reserved, files can
  be locked, paths can be long, the drive may be FAT32 or exFAT, and the drive can vanish
  mid-write. Use `dunce` to avoid `\\?\` paths in anything shown to a user.

---
trigger: glob
globs: "**/*.rs, **/Cargo.toml, crates/**, programs/**/src-tauri/**"
description: Rust, Tauri, portability and vault rules
---
Follow `.claude/rules/rust-standards.md`, `.claude/rules/portability-no-trace.md` and, in
`crates/vault`, `.claude/rules/vault-security.md` (read them first).
@../../.claude/rules/rust-standards.md
@../../.claude/rules/portability-no-trace.md
@../../.claude/rules/vault-security.md
Hard rules: Windows only, no `unwrap` or `expect` outside tests, no `unsafe`, every path from
`crates/paths`, nothing written outside installDir, vault secrets never logged, IPC takes ids.

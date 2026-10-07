# Decisions

## 2026-10-07 Task 1 (foundation)

- **Pins re-verified, no version changes.** Every npm catalog entry in `research/versions.md` §3b (core and optional) was checked with `bun pm view <pkg> version` and every crate in §3c with `cargo search` plus the crates.io `max_stable_version` (cargo search shows Tauri 3 alphas and notify 9 rcs first; those are pre-releases). All matched the research pins: bun 1.4.2, turbo 2.11.7, Rust 1.99.0, Tauri 2.12.1 / tauri-build 2.7.1, plugins opener 2.7.0, global-shortcut 2.4.0, os 2.4.0, log 2.10.0, single-instance 2.5.2, vite 8.3.3, @vitejs/plugin-react 6.1.2, @babel/core 8.0.7, lefthook 2.2.0, knip 6.40.0, cspell 10.3.6, biome 2.5.15, zeroize 1.9.1.
- **New pins (not in the research tables):** `fflate` 0.8.3 (npm, Task 13 zip writer), `rfd` 0.17.2 (crate, Task 12 fatal message box). `aead-stream` 0.6.0, `blake2` 0.11.0, `unicode-normalization` 0.1.25, `proptest` 1.11.0 come from `research/vault.md` §1.
- Cargo requirements are caret (`"2.12.1"`), made exact by the committed `Cargo.lock` and `--locked` (versions.md §3c), not `=` pins.
- Crates have no `package.json` (spec A1); Rust runs as `//#rust:*` root turbo tasks. `turbo.json` is `research/turborepo.md` §2.2 verbatim (already has `agentGuidance: false`).
- `genslate-launcher` (src-tauri) drops `genslate-design-tokens` (unused per port-map §4a) and adds `genslate-vault`, `rfd`, `secrecy` (serde), `notify`, `notify-debouncer-full`, `winreg` (Windows target only); no direct `windows`/`windows-sys`.
- `.env.example` drops `GENSLATE_PORTABLE`: paths now only has Dev and Suite modes.
- `src-tauri/build.rs` and `src/main.rs` hold `fn main() {}` placeholders so the workspace compiles until Task 12.

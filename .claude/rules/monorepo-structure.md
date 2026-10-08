---
paths:
  - "turbo.json"
  - "package.json"
  - "**/package.json"
  - "Cargo.toml"
  - "**/Cargo.toml"
  - "crates/**"
  - "packages/**"
  - "programs/**"
  - "scripts/**"
---

# Monorepo structure

Turborepo orchestrates TypeScript tasks per package and Rust tasks once at the root; bun is the
package manager; Cargo is one workspace. Only the lead edits root manifests (`package.json`,
`turbo.json`, `Cargo.toml`, lockfiles); ask before changing one.

## Where things go

| Path | Holds | May depend on |
|---|---|---|
| `crates/paths` | installDir discovery, the only source of paths | std and small crates |
| `crates/design-tokens` | generated Rust token constants | nothing |
| `crates/launcher-core` | launcher logic: catalog, config, launch, recent | `paths` |
| `crates/vault` | encrypted storage | RustCrypto crates |
| `crates/testing` | shared test helpers (no-trace guard, temp trees) | dev-only |
| `packages/tokens` | token source and generators | nothing |
| `packages/design-system` | React components | `tokens`, Base UI, no Tauri |
| `packages/tauri-bridge` | typed IPC with browser fallbacks | `@tauri-apps/api` |
| `packages/config-*` | shared tsconfig and Vite presets | none |
| `programs/desktop/<app>` | a Tauri app (`src/` UI, `src-tauri/` shell) | packages, core crates |
| `programs/webapp/example` | the SLATECORE Design Kit | `design-system`, `tokens` |
| `scripts/` | root commands, libs, templates, tests | none of the above |

Dependencies point down the table: libraries never import apps, `design-system` never imports
`tauri-bridge` or Tauri, crates never import Tauri (only `src-tauri` does).

## Rules

- A new app comes from `bun run new-app <name>` (see the `new-app` skill), never by copying a folder.
- Shared UI belongs in `design-system`, shared logic in a crate or package; do not duplicate it in
  an app. Apps keep their own feature code under `src/features/<feature>/`.
- Workspace packages use `workspace:*` and `catalog:`; crates use `workspace = true`.
- Generated output (`dist/`, `target/`, `.turbo/`, `src-tauri/gen/`) is never committed; see
  `.gitignore`. installDir runtime folders are ignored except their `.gitkeep` files.
- Turbo tasks are in `turbo.json`; run one package with `--filter=@genslate/<name>`. Root Rust tasks
  are `//#rust:fmt`, `//#rust:lint`, `//#rust:test`, `//#rust:machete`, `//#rust:deny`.

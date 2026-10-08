# Active context

Loaded into every Claude session through `CLAUDE.md`, so keep it under 40 lines and rewrite it
rather than appending. Last updated 2026-10-08 (launcher UI fix round 2).

## State

- Branch `build/launcher-v1` (pushed), building SLATECORE LAUNCHER by GENSLATE from the approved
  launcher plan (`docs/superpowers/plans/2026-10-07-genslate-launcher-plan.md`, local only).
- Landed: monorepo foundation, tokens, design system, Design Kit, tauri-bridge, root commands
  and git hooks, CI, the agent folders, the crates `paths`, `launcher-core` and `vault` (vault
  still in its review fix round), and the launcher UI (tabs, slash bar, Settings tool, tray
  menu) against the browser mock. `git log --oneline` is the source of truth.
- Remaining: Task 12 (Tauri shell in `src-tauri`, serving the real command registry and IPC),
  Task 13, Task 16, and screenshots of the real app on Windows.
- The UI's command handlers use the Rust registry's ids and parameter names
  (`crates/launcher-core/src/actions.rs`); a launcher test reads that file, so renaming an id or
  parameter there means updating `src/app/launcher-commands.model.ts`.

## Owner rules (always)

1. Plan first: get the owner's approval before doing any work; beyond the approved launcher
   plan, post the plan and wait. Detail in `.claude/rules/workflow.md`.
2. Screenshots: finished UI is captured in Polar Night and Snow Storm as
   `<app>-<polar-night|snow-storm>-<view>.png` in `.claude/project/screenshots/<app>/`,
   committed with the work and shown in the reply.

## Working agreements

- Agents work in parallel on disjoint paths in one checkout. Stage and commit only your own paths
  (`git add -- <paths>`, `git commit -- <paths>`); other agents may have uncommitted files.
- No agent credit in commits or PRs; the commit-msg hook and CI reject it.
- Never delete or rewrite `.claude/project/` (committed screenshots from other agents).
- `bun run check` is the gate; `turbo run build` for the launcher waits on Task 12.

## Open questions

- Which single icon set long term (Codicons today)? Owner has not objected.
- Do Cursor and Antigravity add commit trailers? Unverified; the attribution check covers both.

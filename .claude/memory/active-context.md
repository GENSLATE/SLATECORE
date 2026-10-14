# Active context

Imported by `CLAUDE.md`; keep it under 40 lines and rewrite it, do not append. Last updated 2026-10-12 (launcher v1 ready for owner PR review).

## State

- Branch `build/launcher-v1`, building SLATECORE LAUNCHER by GENSLATE from the approved launcher
  plan (`docs/superpowers/plans/2026-10-07-genslate-launcher-plan.md`, local only).
- Landed: every task of the plan, including the Tauri shell (`src-tauri`), packaging
  (`bun run package`, no-trace Tiers 1 to 3) and the launcher UI. The whole-branch review found
  no Critical issue. `git log --oneline` is the source of truth.
- Waiting on the Windows machine: real build and run, `bun run package` re-run, Tier 2, tray and
  vault-notice checks, and screenshots of the real app (the Linux ones come from the browser mock).
- Owner decisions pending: GENSLATE licence text (release builds refuse while it is empty), whether
  auto-lock may force-wipe unsaved vault edits (today it never does), an About note on WebView2
  HKCU keys, and the empty root README, LICENSE and CHANGELOG.
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
- Never delete or rewrite `.claude/project/` (other agents' screenshots).
- `bun run check` is the gate; the launcher build and Rust shell tests run on the Windows machine.

## Open questions

- Which single icon set long term (Codicons today)? Owner has not objected.
- Do Cursor and Antigravity add commit trailers? Unverified; the check covers both.

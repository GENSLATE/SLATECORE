# Active context

Loaded into every Claude session through `CLAUDE.md`, so keep it under 40 lines and rewrite it
rather than appending. Last updated 2026-10-08 (agent folders task).

## State

- Branch `build/launcher-v1`, building SLATECORE LAUNCHER by GENSLATE from the approved launcher
  plan (`docs/superpowers/plans/2026-10-07-genslate-launcher-plan.md`, local only, gitignored).
- Landed: monorepo foundation, tokens, design system, Design Kit, tauri-bridge, root commands and
  git hooks, CI, the `paths` crate, the first launcher UI (tabs, slash bar, Settings tool) and
  the agent folders. `git log --oneline` is the source of truth.
- In flight at the time of writing: `launcher-core` and `vault` crates (uncommitted work in
  `crates/`), then the Tauri shell, packaging and the portability test.

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
- `bun run check` is the gate. Empty skeleton files owned by unfinished tasks can still fail
  Biome or knip; run the gates on your own files and say so in your report.

## Open questions

- Which single icon set long term (Codicons today)? Owner has not objected.
- Whether Cursor and Antigravity add commit trailers is unverified; the attribution check
  covers both.

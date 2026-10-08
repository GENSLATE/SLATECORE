---
name: check
description: Run the SLATECORE quality gates (bun run check) and report every failure with its cause. Use before saying work is done, or when asked to check, lint or verify the repository.
---

# Check

Run the gates and report. Fix only what the task covers; tell the owner about the rest.

1. Run `bun run check` from the repository root (bun only, never npm). Use `--ts` for the
   TypeScript and repository gates or `--rust` for the Rust gates when the change is one-sided.
2. It covers the lockfile, typecheck, Biome, cspell, knip, Rust format and lint, and the
   agent-folder check (`scripts/lib/agents-check.ts`).
3. For each failure give: the gate, the file and line, the cause, and the fix. Fixes go at the
   source: `bun run format` for formatting, `bun run tokens` for token drift, the text itself for a
   real typo, `.config/cspell/project-words.txt` for a legitimate new word. Never silence a rule,
   loosen a config or edit a generated file.
4. Rerun until green. Some failures belong to files other agents are still writing (empty
   skeleton files can fail Biome or knip): report them, do not fix them.
5. Report which gates ran and passed, what failed and why, and what you changed.

Rules to keep in mind: `AGENTS.md`. Plan first and get approval before building anything new
(`.claude/rules/workflow.md`).

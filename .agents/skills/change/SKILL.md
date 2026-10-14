---
name: change
description: Write a changelog fragment in .changes/unreleased for a user-visible SLATECORE change. Use after finishing a user-visible feature or fix.
---

# Change

Rule: `.claude/rules/changes.md`.

1. Decide if the change is user-visible. Tests, refactors, CI and internal docs are not: write
   nothing and say so.
2. Pick the kind: Added, Changed, Deprecated, Removed, Fixed or Security.
3. Write one sentence in the past tense that a user understands, with brand names in capitals
   (SLATECORE LAUNCHER, GENSLATE).
4. Create `.changes/unreleased/<Kind>-<yyyymmdd>-<hhmmss>.yaml` with three keys: `kind`, `body`,
   `time` (ISO 8601, for example `2026-10-08T12:00:00.000000+00:00`).
5. Show the file. Do not edit `CHANGELOG.md` or `.changes/releases/`.

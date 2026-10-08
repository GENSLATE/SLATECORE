---
description: Write a changelog fragment in .changes/unreleased for a user-visible change
argument-hint: "<Added|Changed|Deprecated|Removed|Fixed|Security> <one sentence>"
allowed-tools: Read, Write, Bash(date *), Bash(git diff *), Bash(git log *)
---

Write one changelog fragment. Rule: `.claude/rules/changes.md`. Arguments: $ARGUMENTS

1. Decide whether the change is user-visible. If not (tests, refactors, CI, internals), say so
   and write nothing.
2. Kind is the first word of the arguments (default: infer it from `git diff`). Body is one
   sentence in the past tense that a user would understand; brand names in capitals.
3. Create `.changes/unreleased/<Kind>-<yyyymmdd>-<hhmmss>.yaml` (use `date -u`) with the keys
   `kind`, `body` and `time` (ISO 8601, for example `2026-10-08T12:00:00.000000+00:00`).
4. Show the file. Leave `CHANGELOG.md` and `.changes/releases/` alone.

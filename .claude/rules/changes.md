---
paths:
  - ".changes/**"
  - "CHANGELOG.md"
---

# Changelog entries

Release notes use changie's file format (`.changie.yaml`). Every user-visible change adds one
fragment in `.changes/unreleased/`; the `/change` command writes it.

- File: `.changes/unreleased/<Kind>-<yyyymmdd>-<hhmmss>.yaml`.
- Content, three keys: `kind` (Added, Changed, Deprecated, Removed, Fixed or Security), `body`
  (one sentence in the past tense, user-facing, no file names), `time` (ISO 8601).
- Not user-visible (tests, refactors, CI, docs of internals): no fragment.
- Never edit `.changes/releases/` or `CHANGELOG.md` by hand; `bun run version` folds fragments in.
- Names in the text follow `branding.md` (SLATECORE LAUNCHER, GENSLATE).

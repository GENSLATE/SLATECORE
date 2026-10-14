# .agents

Antigravity project setup, plus the skills Cursor also loads. Antigravity reads `AGENTS.md` at the
repository root (verify in the Customizations panel; if it is missing, add an `always_on` rule
that points at `../../AGENTS.md`).

- `rules/*.md`: 15-line shims with a valid `trigger` (`always_on`, `model_decision`, `glob`,
  `manual`; a rule without one is dropped) that point at the full text in `.claude/rules/`.
  `workflow.md` is always on: plan first, then screenshots in both themes.
- `skills/<name>/SKILL.md`: `check`, `run-tests`, `ui-fixes`, `change`. Skills are slash-invocable
  in Antigravity and Cursor.
- `mcp_config.json`: Context7 (`serverUrl`).

Not here on purpose: `workflows/` (Antigravity retires them on 2026-11-01; skills replace them),
hooks, personas, memory and logs. Memory and decisions live in `.claude/memory/`.

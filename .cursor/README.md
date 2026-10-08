# .cursor

Cursor is the editor for basic edits, so this folder stays thin. Cursor reads `AGENTS.md` at the
repository root and loads skills from `.agents/skills/` (shared with Antigravity).

- `rules/*.mdc`: 15-line shims. Each has Cursor frontmatter (`description`, `globs`,
  `alwaysApply`) and points at the full text in `.claude/rules/`. `workflow.mdc` is always on
  (plan first, screenshots in both themes); `changes.mdc` is requested by the agent.
- `mcp.json`: Context7 for current library docs. Export `CONTEXT7_API_KEY` in your environment.

Not here on purpose: hooks (lefthook and CI enforce the rules), agents, commands (skills cover
them), `settings.json` and `extensions.json` (not Cursor files; editor recommendations are in
`.vscode/extensions.json`). `.cursorignore` at the root keeps generated and private files out of
the index; it is not a security boundary. Check Settings > Rules after a change.

<!-- cspell:ignore cursorignore -->

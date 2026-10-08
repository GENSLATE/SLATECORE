# .claude

Claude Code project setup for SLATECORE. The shared rules are in `AGENTS.md` (always on);
`CLAUDE.md` imports it. Everything here is checked by `bun run check` (`scripts/lib/agents-check.ts`).

| Path | What it holds |
|---|---|
| `settings.json` | Attribution off, permissions (bun only, no force push, no vault reads), PowerShell tool pinned off, hooks |
| `rules/*.md` | Detail per topic, loaded when Claude touches a matching path (`paths:`). `workflow.md` has no `paths` and is always on. Only `.md` files load |
| `skills/<name>/SKILL.md` | Procedures: `new-component`, `design-tokens`, `new-app`, `release` |
| `commands/*.md` | Slash commands: `/check`, `/change` |
| `agents/*.md` | Reviewers and engineers: `code-reviewer`, `security-reviewer`, `ui-visual-qa`, `design-system-engineer`, `tauri-rust-engineer` |
| `hooks/*.hook.ts` | `session-start`, `guard-generated`, `format-on-edit`; run as `bun <script>` (exec form) and fail open |
| `memory/` | `active-context.md` is imported every session (40 lines); `decisions.md`, `lessons-learned.md`, `specifications.md` are read on demand |
| `project/screenshots/<app>/` | Committed UI screenshots, both themes, reused for READMEs and websites. Never delete or rewrite other agents' files here |

## Adding things

- **Rule:** create `rules/<topic>.md` with `paths:` globs, keep it short, then add a Cursor shim
  in `.cursor/rules/` and an Antigravity shim in `.agents/rules/` that point at it, and list it in
  `scripts/lib/agents-check.ts` (the check fails on a missing or dangling pointer).
- **Skill or command:** `skills/<name>/SKILL.md` with `name` equal to the folder and a `description`
  that says when to use it. Side-effect workflows set `disable-model-invocation: true`.
- **Agent:** `agents/<name>.md` with `name`, `description` and the tools it needs.
- **Hook:** a script in `hooks/` plus an entry in `settings.json` with `"command": "bun"` and the
  script in `args`. Hooks are guard rails; lefthook and CI are the safety net.
- Local overrides go in `settings.local.json` (gitignored). Do not put secrets in any of these files.

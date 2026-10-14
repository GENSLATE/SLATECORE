@AGENTS.md
@.claude/memory/active-context.md

## Claude Code

- Attribution is off in `.claude/settings.json` (empty commit and PR text, no session link) and the
  no-agent-credit rule in AGENTS.md beats the built-in commit and PR instructions: add no trailers
  or footers by hand either.
- Plan first: use plan mode for anything new, post the plan, wait for the owner's approval. When
  UI work is done, screenshot both themes into `.claude/project/screenshots/<app>/` and show them.
- The shell is Git Bash (the PowerShell tool is pinned off, so every `Bash(...)` rule applies).
  Run turbo through `bun x --no-install turbo`.
- Path-scoped rules in `.claude/rules/` load when you read or edit a matching file. Skills:
  `/new-component`, `/design-tokens`, `/new-app` and `/release` (the last two only on request).
  Commands: `/check [--no-fix]` and `/change`.
- Agents in `.claude/agents/`: `code-reviewer` after every diff, `security-reviewer` for vault,
  IPC, paths, capabilities, CI and dependencies, `ui-visual-qa` before UI work is called done,
  `design-system-engineer` and `tauri-rust-engineer` for implementation.
- Hooks in `.claude/hooks/` format edited files, block edits to generated files and keep the git
  identity human. They are guard rails, not enforcement: lefthook and CI are the safety net.
- Memory in `.claude/memory/`: only `active-context.md` is imported (keep it under 40 lines).
  `decisions.md`, `lessons-learned.md` and `specifications.md` are read when needed; add to them
  when you decide something lasting or hit a gotcha.
- Read `active-context.md` first: it says what is in flight and which paths each agent owns.

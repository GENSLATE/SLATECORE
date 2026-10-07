@AGENTS.md

## Claude Code

- Commit and PR attribution is switched off in `.claude/settings.json` (`attribution.commit` and `attribution.pr` are empty). Do not add trailers or footers by hand either.
- Stage only the paths you own (`git add -- <paths>`), never `git add -A` or `git commit -a`.
- Prefer the Bash tool with bun commands; run turbo through `bun x --no-install turbo`.

# Lessons learned

Gotchas with their fix. Add one when something cost you time; keep each to a line or two.

## Tooling

- **Biome needs the config path.** Run it with `--config-path=.config/biome.json`; the config file
  excludes itself from its own `files.includes`.
- **Biome and `package.json` arrays.** `package.json` uses `json.formatter.expand: auto` (see the
  override in the Biome config) so short arrays stay on one line.
- **`useLiteralKeys` fights `noPropertyAccessFromIndexSignature`.** The TypeScript option wins:
  `useLiteralKeys` is off, so write `record['key']`.
- **No `.ts` extensions in bun script imports.** The bundler resolver finds the file, and tsc with
  `moduleResolution: bundler` accepts the bare path.
- **`cargo deny --config ... check`.** The config flag is global and comes before `check`; do not
  alias over the subcommand name.
- **Cargo workspace member globs must match something,** or `cargo metadata` fails.
- **Run the Tauri CLI as `bun run tauri` inside the app folder.** `bun x tauri` elsewhere can fetch
  an unrelated package.
- **Clippy errors can hide behind the progress bar.** Set `CARGO_TERM_PROGRESS_WHEN=never` in CI.
- **Doctests may not link on Windows** because of the `msvcrt.lib` stub `tauri-build` writes; the
  testing crate sets `doctest = false`.
- **Turbo does not hash the Rust compiler,** so `rust-toolchain.toml` pins an exact channel, never
  `stable`.

## Tauri and Windows

- **WebView2 data leaks to `%LOCALAPPDATA%`** unless the window is built in code with
  `data_directory` and `WEBVIEW2_USER_DATA_FOLDER` is set before the first window.
- **`tauri-plugin-window-state` joins its file name onto the OS config dir.** Do not register it.
- **Tauri 2.11 has no atomic window move plus resize.** Reveal content inside a fixed-size frame
  and animate transform and opacity (see `motion-polish.md`).
- **Windows-only failures need a Windows runner.** CI runs on `windows-latest`; Linux CI hides them.
- **Drive letters change** (`S:` to `E:`). Persist relative paths only.

## Git, hooks and agents

- **IDE sync shows only "failed to push some refs"** when a hook fails, and a slow hook looks like a
  hang. The pre-push hook stays fast (`bun run check --ts`); the full suites run in CI.
- **Git hooks are shared by worktrees.** Run `lefthook install` from the main checkout.
- **Several agents share one checkout.** Stage only your own paths with `git add -- <paths>` and
  commit with a pathspec; never `git add -A`.
- **Cloud harnesses inject agent credit** (trailers, session links) into commit text. The
  `attribution` settings, AGENTS.md and the commit-msg hook strip or reject it.
- **Hook rules for Claude Code:** exec form (`"command": "bun"` plus `args`) behaves the same under
  Git Bash and PowerShell; exit 2 blocks, anything else is non-blocking; fail open on bad input.
- **A scoped `Bash(...)` deny rule can switch the PowerShell tool off or leave it unguarded** with
  Git Bash installed. `CLAUDE_CODE_USE_POWERSHELL_TOOL=0` pins it and mirrored `PowerShell(...)`
  deny rules cover the other case.
- **`.claudeignore` does nothing.** Use `permissions.deny` Read rules. `.claude/rules/*.mdc` is never
  loaded; only `.md` is. Antigravity drops a rule whose `trigger` is not valid.
- **Turborepo edits `AGENTS.md`** when it detects an agent unless `agentGuidance` is `false` in
  `turbo.json` (it is).

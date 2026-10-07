# scripts

Every root command is a bun TypeScript file in `commands/`, wired up in the root `package.json`
`scripts`. Run them with `bun run <command>`. Commands with their own flags support `--help`;
the thin turbo wrappers pass every extra argument straight to turbo (`bun run check --force`).

| Command | What it does |
|---|---|
| `bun run setup` | `bun install`; checks the running bun against the `packageManager` pin; installs the toolchain pinned in `rust-toolchain.toml`; installs `cargo-deny` and `cargo-machete` when missing; installs the git hooks (`lefthook install`); checks MSVC build tools and the WebView2 runtime. |
| `bun run dev [app] [--web] [--kit]` | Always one filtered package, never an unfiltered `turbo run dev`. No flag: the Tauri launcher. `--web`: the launcher UI in a browser with the mock backend. `--kit`: the Design Kit on `http://localhost:1430`. `dev notes --web`: an app made with `new-app`. |
| `bun run build` | `turbo run build`. |
| `bun run package` | Placeholder that exits with an error until the portable zip packaging lands. |
| `bun run test` | `turbo run test //#rust:test --continue`, then `bun test scripts/tests`. |
| `bun run check [--ts] [--rust]` | A lockfile drift check (`bun install --frozen-lockfile --dry-run`), then `turbo run typecheck check //#biome //#spell //#knip //#rust:fmt //#rust:lint //#rust:machete //#rust:deny --continue`, the root TypeScript check (`tsc` over `scripts/` and tooling) and the agent-folder drift check (`lib/agents-check.ts`). `--ts` and `--rust` run one half. |
| `bun run format` | `turbo run //#biome:fix //#rust:fmt:fix` (both uncached). |
| `bun run tokens [--check]` | `turbo run generate --filter=@genslate/tokens`; `--check` runs the drift check instead. |
| `bun run version <patch\|minor\|major\|x.y.z> [--dry-run]` | Bumps every `package.json`, `[workspace.package]` in `Cargo.toml` and literal `tauri.conf.json` versions, then `cargo update --workspace` and `bun install`. |
| `bun run new-app <name>` | Scaffolds `programs/desktop/<name>` and `crates/<name>-core` from `scripts/templates`, adds both crates to the Cargo workspace, runs `bun install` and generates the bundle icons. |
| `bun run attribution [--message <file> [--fix]] [--range <a..b>] [--branch <name> \| --current-branch] [--body-env <VAR>]` | Rejects commits, branches and PR bodies that credit an agent as author (git hooks and CI). |
| `bun run clean [--deep] [--keep-target] [--dry-run]` | Removes `.turbo`, `dist`, `out`, `coverage`, `*.tsbuildinfo`, Tauri `gen/`, `release/*` and the contents of `target/`. `--deep` also removes every `node_modules`. `release/.archive/` and the tracked `release/.gitkeep` and `target/.gitkeep` are never removed. |

## Layout

```text
scripts/
├── commands/        one file per root command (turbo.util.ts is the shared turbo helper)
├── lib/             pure helpers, each taking the repo root where tests need a temp copy
│   ├── agents-check.ts   agent-folder drift check (stub until the agent-folders task)
│   ├── args.ts           defineCommand(): parsing, --help, exit codes
│   ├── attribution.ts    the authorship rules behind `bun run attribution`
│   ├── clean.ts          what `clean` removes
│   ├── generated-paths.ts  GENERATED_GLOBS: files tools write and people never edit
│   ├── log.ts            coloured output (respects NO_COLOR / FORCE_COLOR)
│   ├── new-app.ts        scaffolding and Cargo workspace edits
│   ├── paths.ts          repo root
│   ├── run.ts            Bun.spawn wrapper (inherited stdio, exit codes, capture)
│   ├── template.ts       renderer for the Tera subset the templates use
│   ├── toolchain.ts      bun and Rust pin checks used by `setup`
│   ├── turbo.ts          how the pinned turbo is started
│   └── version.ts        version bump plan
├── templates/       sources for `new-app`; every file ends in `.tera` so linters and
│                    compilers never see unrendered tags
└── tests/           bun tests (`bun test scripts/tests`)
```

## Git hooks

`lefthook.yml` defines the hooks; `bun run setup` installs them.

- `pre-commit`: biome (`--write`, re-stages fixes), rustfmt, the token drift check when token
  sources change, and cspell. Spelling is advisory here (`--no-exit-code`) so a new word never
  blocks a commit; `bun run check` and CI fail on it. Add legitimate words to
  `.config/cspell/project-words.txt`.
- `commit-msg`: commitlint (scopes in `.config/commitlint.config.ts`) and
  `bun run attribution --message`.
- `pre-push`: `bun run attribution --range origin/main..HEAD --current-branch` and
  `bun run check --ts`.

Skip once with `LEFTHOOK=0 git commit ...`. CI runs the same checks.

## Authorship rule

No commit message, branch, author or committer may credit an agent. `bun run attribution`
rejects `Co-authored-by` / `Signed-off-by` trailers and `Claude-Session:`-style trailers naming
claude, anthropic, cursor, cursoragent, gemini, antigravity, copilot, codex, openai, chatgpt,
devin, windsurf, aider or jules; "Generated with ..." footers; agent session links; and branches
starting `claude/`, `cursor/`, `codex/`, `copilot/`, `devin/`, `jules/` or `antigravity/`. A human
`Co-authored-by` (for example a gmail address) passes, and plain mentions of `.claude/` or
`CLAUDE.md` pass. `--message <file> --fix` strips the offending lines from the message file.

## Expected `cargo machete` findings (TODO until the owning tasks land)

`cargo machete` cannot be configured from this folder, so the dependencies the workspace
declares ahead of their first use will be reported until the code that uses them exists. Re-run
it after the crates are implemented and move anything that is still reported either into the
code that uses it or into `[workspace.metadata.cargo-machete] ignored` in the root `Cargo.toml`
(owned by the workspace task):

- `programs/desktop/launcher/src-tauri`: `tauri-plugin-opener`, `notify`, `notify-debouncer-full`,
  `rfd`, `winreg`, `secrecy` and the other plugin crates until the shell wires them.
- `crates/vault`: `subtle` (declared in the workspace, not yet in a crate), `serde_json`,
  `zeroize`, `unicode-normalization` until the vault code uses them.
- `crates/launcher-core`: `notify`, `nvml-wrapper` (optional `nvidia` feature),
  `pelite`, `ico`, `rust-ini`, `sysinfo`, `toml_edit` until the catalog and system modules land.
- Test-only crates (`proptest`, `tempfile`, `genslate-testing`) in `[dev-dependencies]` are
  reported only while no test uses them.

The matching knip entries for npm packages are in `.config/knip.json` (`ignoreDependencies`).

## Adding a command

1. Create `commands/<name>.ts`; use `await defineCommand({ name, summary, options, run })` when it
   has flags, or `runTurbo()` from `commands/turbo.util.ts` for a plain turbo delegation.
2. Add `"<name>": "bun scripts/commands/<name>.ts"` to the root `package.json` `scripts`.
3. Document it in the table above and add a test in `tests/`.

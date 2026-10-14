---
name: new-app
description: Scaffold a new SLATECORE suite app (Tauri 2 desktop app plus its core crate) with bun run new-app. Use only when the owner asks for a new app.
disable-model-invocation: true
argument-hint: <name>
---

# New SLATECORE app

A new app is a scope change: it needs the owner's approval of a written plan before you build it
(`.claude/rules/workflow.md`). The plan names the app, what it does, its installDir folder and
which design-system components it needs.

## Scaffold

```
bun run new-app $ARGUMENTS [--title "<Name>"] [--description "<one line>"]
```

This writes `programs/desktop/<name>` and `crates/<name>-core` from `scripts/templates`, registers
both in the Cargo workspace, runs `bun install` and generates the bundle icons. The window title
is "SLATECORE <Title>", the Cargo package `genslate-<name>`, the binary `slatecore-<name>`, the npm
package `@genslate/<name>` and the identifier `xyz.genslate.slatecore.<name>` (names:
`.claude/rules/branding.md`). Flags `--identifier` and `--port` override the defaults; the port
pair starts at 1440.

## After scaffolding

1. Ask the owner to add the app's commit scope to `.config/commitlint.config.ts` (and its entry
   points to `.config/knip.json` if they differ); those root configs are not yours to edit.
2. Read the generated `README.md`; keep the app on the shared packages: UI from
   `@genslate/design-system`, IPC through `@genslate/tauri-bridge`, tokens through the shared CSS
   import. No app-specific colours, no copied components.
3. Where it installs: `programs/genslate/<name>/` inside installDir, with state under
   `other/<name>/`. It follows `.claude/rules/portability-no-trace.md` from the first line: paths
   from `genslate-paths`, nothing written outside installDir.
4. Start with a failing test (bun test for UI, cargo test for the core crate), then build.
5. Run the gates: `bun run check` and `bun run test`.
6. Run `bun run dev <name> --web` and review every view in both themes. Screenshots go to
   `.claude/project/screenshots/<name>/` as `<name>-<polar-night|snow-storm>-<view>.png`, are
   committed with the work and shown in your reply.
7. Ask `security-reviewer` to review capabilities, CSP and IPC before the app is called done.

## Do not

- Copy another app's folder by hand, or edit `scripts/templates` for one app's needs.
- Register OS-touching Tauri plugins (dialog, updater, opener defaults, deep-link, store,
  window-state).
- Edit a root manifest or add a dependency without asking the owner (the script registers the new
  app in `Cargo.toml` and the lockfiles itself).

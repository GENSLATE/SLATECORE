---
paths:
  - "**/*.ts"
  - "**/*.tsx"
---

# TypeScript standards

- **Strictest compiler.** Configs extend `@genslate/config-typescript` (`strict`,
  `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noPropertyAccessFromIndexSignature`,
  `erasableSyntaxOnly`, `verbatimModuleSyntax`). Fix the types, do not loosen the config.
- **No `any`, no non-null `!`, no enums, no default exports** (config files that need one are
  listed in `.config/biome.json`). Use `unknown` and narrow it, unions and `as const` objects, and
  `import type` / `export type` for types.
- **Named exports** and dotted kebab-case file names `<subject>.<kind>.<ext>`: `.component.tsx`,
  `.types.ts`, `.variants.ts`, `.hook.ts`, `.util.ts`, `.test.ts(x)`. Folders are kebab-case.
- **React 19 function components** with `ref` as a prop. The React Compiler is on: write plain
  code, no `useMemo`, `useCallback` or `memo` unless a measured problem needs one.
- **Format and lint with Biome** (`bun run format`; the format-on-edit hook does it too): single
  quotes, semicolons, trailing commas, 100 columns, organised imports.
- **Tokens, not values.** Styling follows `design-contract.md`; user-visible text follows
  `branding.md`.
- **Scripts** (`scripts/`, `.claude/hooks/`): bun TypeScript run as `bun path/to/file.ts`. Imports
  have no `.ts` extension. Spawn processes with an argv array (`Bun.spawn([...])`), never a shell
  string. Pure functions take their inputs (and the repo root) as parameters so tests can pass a
  temp folder. One `scripts/commands/<name>.ts` per root command.
- **Tests:** `bun test` (happy-dom for UI), files named `*.test.ts(x)`, descriptive test names
  (`snake_case` in `scripts/tests`). See `testing.md`.
- **Versions:** pinned in the root `package.json` catalog (`catalog:` in packages). Do not add a
  dependency without asking; `bun.lock` is generated.
- **Type check:** `bun run check --ts` (or `bun x --no-install tsc --noEmit -p tsconfig.json`
  inside a package).

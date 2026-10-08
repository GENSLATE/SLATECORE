# SLATECORE LAUNCHER: tests

- `tests/unit/**/*.test.ts(x)`: bun test + Testing Library + happy-dom.
- `tests/setup/dom.preload.ts` registers happy-dom (the `test` script passes `--preload`).
- `tests/unit/launcher.harness.tsx` renders the whole launcher on the browser mock backend
  (`src/ipc/launcher.mock.ts`), with spies on the calls a test wants to check.

Run with `bun run test` in this folder, or
`bun x --no-install turbo run test --filter=@genslate/launcher` from the repo root.

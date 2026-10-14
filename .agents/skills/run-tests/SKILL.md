---
name: run-tests
description: Run the narrowest useful SLATECORE test target for a change and diagnose failures in a fixed format. Use when asked to run tests or when a test fails.
---

# Run tests

Pick the narrowest target that covers the change, then widen.

| Changed | Run |
|---|---|
| One TypeScript package | `bun x --no-install turbo run test typecheck --filter=@genslate/<name>` |
| `scripts/` or agent folders | `bun test scripts/tests` (one file: `bun test scripts/tests/<file>.test.ts`) |
| One Rust crate | `cargo test -p genslate-<crate> --locked` |
| Everything | `bun run test` (turbo tests, Rust tests, then `scripts/tests`) |

Packages: `tokens`, `design-system`, `tauri-bridge`, `launcher`, `example`. Crates: `paths`,
`design-tokens`, `launcher-core`, `vault`, `testing`. bun only, never npm.

## When a test fails

Diagnose before you change anything, and report in this format:

1. **Failing test:** file, test name, one-line symptom.
2. **Expected vs actual:** the assertion and the values.
3. **Cause:** the line in the code under test (or in the test) that explains it. Reproduce it
   with the single test, not the whole suite.
4. **Proposed fix:** the smallest change, and whether it is a code bug or a wrong test.

Fix only when asked (or when the task is the fix). A bug fix starts with a test that fails for
the reported reason. Never delete, skip or loosen a test to get green. Report the final command
and its result.

Rules: `.claude/rules/testing.md`. Linux cloud sessions cannot build the Windows app: say which
Windows-only tests did not run.

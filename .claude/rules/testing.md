---
paths:
  - "**/*.test.ts"
  - "**/*.test.tsx"
  - "**/tests/**"
  - "crates/*/tests/**"
---

# Testing

- **Write the failing test first,** run it and see it fail for the right reason, then implement.
  A bug fix starts with a test that reproduces the bug. Tests ship in the same commit.
- **TypeScript:** `bun test`, with happy-dom for components (each package preloads
  `tests/setup/dom.preload.ts`). `bun run test` runs turbo tests, Rust tests, then
  `bun test scripts/tests`. One package: `bun x --no-install turbo run test --filter=@genslate/<name>`.
- **Rust:** `cargo test -p <crate> --locked`; shared helpers in `genslate-testing`. Fixtures are
  synthetic temp trees, never the real installDir, never real vault data.
- **Required suites:** token drift and Nord colour checks, the portability test
  (`portability-no-trace.md`), vault round trip, wrong password, tamper and crash recovery
  (`vault-security.md`), slash-command parser tests, config fallbacks (invalid TOML keeps the
  previous settings), and the agent-folder check in `scripts/tests/agents-check.test.ts`.
- **Test behaviour, not implementation:** query by role and label, assert what a user sees, cover
  keyboard paths and every state of a component. Do not snapshot markup.
- **Deterministic:** no real clock, network, drive letters or machine-specific paths; inject them.
  A flaky test is a bug, fix or delete it, do not retry it.
- **Check before claiming done:** run the narrowest test target while working, then
  `bun run check` and `bun run test`; report what you ran and its result.

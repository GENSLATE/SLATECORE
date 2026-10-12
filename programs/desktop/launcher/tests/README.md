# SLATECORE LAUNCHER: tests

- `tests/unit/**/*.test.ts(x)`: bun test + Testing Library + happy-dom.
- `tests/setup/dom.preload.ts` registers happy-dom (the `test` script passes `--preload`).
- `tests/unit/launcher.harness.tsx` renders the whole launcher on the browser mock backend
  (`src/ipc/launcher.mock.ts`), with spies on the calls a test wants to check.

Run with `bun run test` in this folder, or
`bun x --no-install turbo run test --filter=@genslate/launcher` from the repo root.

## Reaching a state in the browser preview (`bun run dev --web`, http://localhost:1420)

The mock backend reads flags from the URL (`mockOptionsFromUrl` in `src/ipc/launcher.mock.ts`:
`theme`, `size`, `pinned`, `status`, `vault`, `empty`, `noThirdParty`, `issue`, `lockBlocked`,
`undeletable`, `foreign`, `idleLock`, `hideLock`). Views that need no flag, only input:

- **Search with no results:** type `nonexistent` into the search box ("No apps match ...").
- **Vault setup error:** open `/?vault=uninitialized`, open Settings (the gear in the rail), choose
  Vault, type `short` in both password fields and press Enter ("Too short: use at least 8 bytes").
- **Window context menu:** right-click the titlebar (the strip with the wordmark and the pin and
  hide buttons); the status bar and a rail folder have their own variants.
- **Vault blocked-type note:** `/?vault=unlocked`, Settings, Vault, click `backup.ps1`.

Add `&theme=snow-storm` (or `polar-night`) to any of these.

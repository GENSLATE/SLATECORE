# Releasing SLATECORE LAUNCHER by GENSLATE

`bun run package` writes the portable zips into `release/`. It never uses an installer: the
launcher is portable, and an installer would write to the PC.

## What it builds

- `slatecore-launcher-<version>-base.zip`: the install folder layout, ready to unzip onto a
  drive. `programs/genslate/launcher/slatecore-launcher.exe`, the seeds in `other/launcher/`
  (settings, documents, licenses, icons), the empty `cache`, `logs` and `database` folders, the
  `storage/users/shared` folders, `storage/vault`, `programs/portableapps.com`,
  `programs/portapps.io` and a README. Zip tools drop empty folders, so each empty folder holds a
  zero byte `.gitkeep`, except `storage/vault`: the vault counts any other file in its folder as a
  plain file waiting to be encrypted, so it ships as a folder entry only.
- `slatecore-launcher-<version>-webview2-runtime.zip` (optional, `--webview2 <cab>`): the Fixed
  Version WebView2 runtime in `other/shared/webview2/<version>-x64/`, to unzip over the same
  folder on PCs without the WebView2 runtime.
- Older builds move to `release/.archive/` once the new zips exist.

The version is the root `package.json` version (`bun run version <bump>` changes it everywhere).
Only an allowlist of seed files is read from the checkout. Runtime state, build output,
screenshots, `.env*` and the `.genslate` folder never go in; the packager checks the finished
list again and refuses to write a zip that has anything else.

## Commands

- `bun run package`: builds with `tauri build --no-bundle`, then packages. Windows only.
- `bun run package --no-build`: package the exe that is already built (`target/release`).
- `bun run package --exe <path>`: package a given exe (no build). Works on any OS, so a build from
  a Windows machine can be packaged elsewhere.
- `bun run package --webview2 <cab>`: also write the runtime add-on (see below).
- `--out <dir>` writes to another folder than `release/`.

## The WebView2 runtime add-on

The add-on is Microsoft's software and is taken only from Microsoft. `scripts/webview2.lock.json`
pins the exact file. It ships with `UNRESOLVED` values, and the add-on step refuses to run until
they are filled in:

1. On the WebView2 download page of Microsoft, download the Fixed Version x64 `.cab` (read the
   license dialog there first).
2. Note the exact runtime version (for example the digits in the file name
   `Microsoft.WebView2.FixedVersionRuntime.<version>.x64.cab`), the address you downloaded it
   from (an https `microsoft.com` address) and the file name.
3. Compute the SHA-256 and the size of that file, for example in PowerShell:
   `Get-FileHash <file>.cab -Algorithm SHA256` and `(Get-Item <file>.cab).Length`. Use the
   lowercase hex digits.
4. Put `version`, `fileName`, `url`, `sha256` and `size` into `scripts/webview2.lock.json`.
5. Keep the cab (Microsoft only keeps the latest versions online), but do not commit it.
6. `bun run package --no-build --webview2 <file>.cab`. The packager checks the file against the
   lock, expands it with Windows' own `expand.exe`, and writes the add-on with a notice from
   `other/launcher/licenses/webview2.NOTICE.md` and a `runtime.json` that records the source.

Measure the add-on once: the extracted runtime is several hundred megabytes.

## Checks

- `bun test scripts/tests/package.test.ts` runs everywhere (a fake exe stands in for the real one).
- `bun test scripts/tests/no-trace.test.ts` runs the portability test on Windows against the
  zip in `release/` and skips elsewhere. See `portability.md`.

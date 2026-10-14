# Portability and leaving no trace

SLATECORE LAUNCHER by GENSLATE runs from a USB drive or an SSD. Everything it controls stays in
the install folder; nothing is written to the registry, `%APPDATA%`, `%LOCALAPPDATA%`, the Start
Menu or the PC's real `%TEMP%`, and nothing starts by itself. Paths are found from the exe's
place (`crates/paths`), never hard-coded, and no absolute path is saved, because the drive letter
changes between PCs.

## What it cannot control

Windows keeps Recent files and jump lists, Prefetch files and the list of tray icons. Apps from
PortableApps.com and portapps.io, and programs the launcher starts, write what they write.
WebView2 and the graphics driver may keep shader caches, crash reports and diagnostic data in
Windows' own folders.

## The test, in three tiers

1. **Rust** (`programs/desktop/launcher/src-tauri/tests/no_trace.rs`): runs the start-up steps
   (find the folders, create them, load settings, scan, open, create, lock and unlock the vault,
   save a setting) against an install folder in a temp dir, and fails if `%TEMP%`, `%APPDATA%` or
   `%LOCALAPPDATA%` changed. Run on Windows with
   `cargo test -p genslate-launcher --locked --test no_trace`.
2. **The packaged exe** (`scripts/tests/no-trace.test.ts`, Windows only): unpacks the zip from
   `release/`, starts the exe, waits until it logs that it started, ends it, and compares
   `%TEMP%`, `%APPDATA%`, `%LOCALAPPDATA%` and `HKCU\Software` before and after with a control run
   that waits the same time with no launcher. What the launcher run changed beyond the control
   run is a leak. It does not open Settings or use the vault (nothing can click a tray app
   from a test) and ends the exe by terminating it: use the app by hand on the PC and compare
   the same folders to cover that. Run it after `bun run package`: `bun test scripts/tests/no-trace.test.ts`.
3. **Process Monitor** (`scripts/no-trace/procmon.ts`, nightly): a trace of a real session is
   checked against `scripts/no-trace/allowlist.json`. Every successful write by the launcher or a
   process it started, outside the install folder, needs a rule with a reason. The capture and
   the check are described at the top of `procmon.ts`.

A change that adds a file, a folder, an environment variable or registry access must extend these
tests.

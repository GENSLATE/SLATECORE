---
paths:
  - "crates/paths/**"
  - "crates/launcher-core/**"
  - "programs/desktop/*/src-tauri/**"
  - "programs/desktop/launcher/installDir/**"
  - "scripts/commands/package.ts"
---

# Portable and leave no trace

SLATECORE LAUNCHER runs from an SSD or USB drive. Plug it into any Windows PC, use it, unplug it:
nothing the launcher controls remains on that PC.

- **Every path comes from `crates/paths`.** installDir is found by walking up from the executable
  to the folder that holds `programs/`, `other/` and `storage/` (dev uses the mock installDir).
  No hard-coded drive letters or user folders. Store paths relative to installDir: the drive letter
  changes between PCs (`S:` to `E:`), and paths can contain spaces, Unicode or be UNC. An absolute
  path is never persisted.
- **No fallback.** If installDir is unwritable or UNC, refuse to run with a clear message. Never
  fall back to `%APPDATA%`, `%LOCALAPPDATA%` or `%TEMP%`.
- **Before the first window opens:** point TEMP and TMP, the WebView2 user data folder
  (`WEBVIEW2_USER_DATA_FOLDER`, and the window's `data_directory`, built in code), and every
  config, cache, log and database directory into installDir. Webviews are incognito. Scrub
  `WEBVIEW2_*` from the environment of apps the launcher starts.
- **Forbidden:** registry writes, the real `%APPDATA%`, `%LOCALAPPDATA%` and `%TEMP%`, Start Menu
  entries and shortcuts, autostart, and plugins that default to OS folders. Lessons: the
  window-state plugin joins its file name onto the OS config dir, and WebView2 defaults to
  `%LOCALAPPDATA%`; neither is registered or left on defaults.
- **Reading is fine:** the launcher may read the registry or OS folders to find tools (for
  example, an installed WebView2 runtime). It never writes there.
- **Windows facts:** case-insensitive paths, reserved names (`con`, `nul`, `aux`, `com1`), file
  locks, long paths, FAT32 and exFAT (no symlinks, no ACLs, 4 GB file limit), a drive pulled
  mid-write. Writes that must not tear use write-to-temp-in-installDir then rename.
- **The portability test is a gate.** Tier 1 snapshots the OS folders around startup in a Rust
  test; tier 2 runs the packaged exe against a control run. Run it, and extend it, for any change
  that adds a file, folder, environment variable or registry access.
- **State the limits** (About and README do): Recent files, Prefetch, tray icon settings and
  whatever PortableApps.com and portapps.io apps write are outside the launcher's control.

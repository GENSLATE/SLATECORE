# Specifications (condensed design spec)

The full spec and plan live in `docs/superpowers/` (gitignored, local only); this is the part agents
need in the repo. Spec date 2026-10-07, plus the amendments (A1 to A13) the owner confirmed.

## Product

SLATECORE LAUNCHER by GENSLATE: a portable, Windows-only app launcher that works like a small
operating system on an SSD or USB drive. Modern reproduction of the PortableApps.com launcher with
its own character. Launches GENSLATE apps first, then PortableApps.com and portapps.io apps, and
gives quick access to the user's files. Also establishes the reusable Nord design system and the
SLATECORE monorepo that later apps share. Success: plug in, use, unplug, and nothing remains on the
PC that the launcher controls.

## Decisions

- Port and evolve SlateSuite code (tokens, design system, bridge, crates, launcher). SlateSuite is
  a reference and is never modified.
- Turborepo runs TypeScript per package and Rust at the root (`//#rust:*`); crates have no
  `package.json`. bun only. Latest stable, exact pins in one catalog. Windows only.
- Own window plus tray icon. Tray left click toggles (300 ms debounce); right click opens a custom
  tray menu window built from the same command registry as the slash bar.
- v1: scan and launch installed apps from three sources, tabs, slash commands, documents rail, and
  a real Settings tool. Everything else is a "Feature Coming Soon" teaser.
- Only `storage/vault` is encrypted. WebView2: host runtime first, else a bundled fixed runtime;
  its data folder is always inside installDir.
- Theme: official Nord (Polar Night dark, Snow Storm light, plus system). Modern flat UI.

## Portable runtime

- installDir is found by walking up from the exe to the folder holding `programs/`, `other/`,
  `storage/`. Dev uses `programs/desktop/launcher/installDir`.
- Inside installDir: `programs/{genslate,portableapps.com,portapps.io}/<app>/`;
  `other/launcher/{cache,configs,database,documents,licenses,logs,resources}`;
  `storage/users/shared/{Desktop,Documents,Downloads,Music,Pictures,Videos}`; `storage/vault/`.
- Config: `other/launcher/configs/settings.toml` and `keybindings.toml`. Invalid TOML keeps the
  previous settings and shows an issue message.
- Leave no trace: no autostart, no registry writes, no OS-folder fallback, no persisted absolute
  path. TEMP/TMP and WebView2 data are redirected before the first window. Limits stated plainly:
  Recent files, Prefetch, tray icon settings and what third-party apps write.

## Vault (A3)

Random 256-bit vault key wrapped by the Argon2id key (m=131072 KiB, t=3, p=1). Files are
write-once blobs `files/<32 hex>.gvf` (streaming XChaCha20-Poly1305); names only in an encrypted
index; header and index have A/B generation slots. Decrypted files live in a session folder inside
installDir, removed on lock, exit and hide timeout, and wiped at the next start if stale. Auto-lock
after 10 minutes idle. Plain files dropped into `storage/vault` are encrypted at the next unlock.
Password 8 to 1024 bytes after NFKC. No recovery. Wrong attempts are rate limited.

## Launcher UI

- Frameless, transparent, fixed bottom-right of the monitor work area, not movable, hidden from the
  taskbar, single instance per drive, refuses to run elevated. Widens with an animation when a tool
  opens; Esc returns and shrinks.
- Left rail: Desktop, Documents, Downloads, Music, Pictures, Videos, Vault. Right panel: pill tabs
  GENSLATE, portapps.io, PortableApps.com with counts; Favorites, Recent, all apps, a collapsed
  "Unavailable" group. Search bar at the bottom; `/` makes it a command line (`/open`, `/theme`,
  `/size`, `/pin`, `/settings`, `/vault`, `/rescan`, `/help`; `/ask` shows the AI teaser).
- Status bar: drive name, free space, temperatures and usage. Zero apps or one source: designed
  empty states and a single-source heading, never a blank panel.
- Settings: Appearance (theme, size S/M/L), Behavior (hide on blur, hide on launch, pinned),
  Keybindings, Vault, About ("SLATECORE LAUNCHER", "by GENSLATE", version, drive, limits).
  App manager, Storage and backup, Diagnostics and AI tools are teasers.
- Dev ports: launcher 1420, Design Kit 1430. A mock backend runs the UI in a browser.

## Design system

`packages/tokens` generates CSS variables, the Tailwind theme, TypeScript, JSON and Rust constants
from one definition; a test checks every colour against Nord. Components (49 folders, recipes,
hooks, providers) on Base UI, Tailwind v4 and tailwind-variants; token classes only. Motion and
polish are requirements (see `.claude/rules/motion-polish.md`). The Design Kit is the review gate.

## Delivery and out of scope

`bun run package` builds the portable zip(s) into `release/` (older builds to `release/.archive/`),
optionally with the bundled WebView2 zip. GitHub Actions on Windows runs check, test and package.
Out of scope for v1: AI beyond teasers, installing or updating third-party apps, the App manager,
Storage and backup, Diagnostics, non-Windows platforms, other suite apps (only `new-app`).

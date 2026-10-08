---
paths:
  - "**/*.{tsx,ts,rs,md,json,toml}"
---

# Branding and names

Check every user-visible string against this before you write it (spec section 1 "Names" and A13).

| Level | Name | Where it shows |
|---|---|---|
| Developer and publisher | **GENSLATE** | About ("by GENSLATE"), README, licence and copyright lines, package `author` fields, the first app tab and the `programs/genslate/` folder, the npm scope `@genslate/*`, the crate prefix `genslate-`, the bundle identifier prefix `xyz.genslate` |
| Suite and monorepo | **SLATECORE** | Root package `slatecore`, `AGENTS.md` and docs headings, the Design Kit title ("SLATECORE Design Kit"), the scaffold of every later app ("SLATECORE <App>") |
| This product | **SLATECORE LAUNCHER** (by GENSLATE) | Window title, tray tooltip and tray menu header, About, README, `slatecore-launcher.exe`, release zips `slatecore-launcher-<version>-*.zip`, bundle identifier `xyz.genslate.slatecore.launcher` |

## Rules

- Brand names are always written in capitals: SLATECORE, GENSLATE. Never "Slatecore" or "Genslate"
  in text a person reads.
- User-visible text says "SLATECORE LAUNCHER" and, where there is room (About, README, licence),
  "by GENSLATE".
- The code namespace stays GENSLATE because it names the developer, not the product:
  `@genslate/*` packages, `genslate-*` crates, `programs/genslate/`, the GENSLATE tab,
  `xyz.genslate.*`. Lowercase identifiers are code, not brand text.
- Fixed identifiers: root package `slatecore`; Tauri package `genslate-launcher` with binary
  `slatecore-launcher`; later apps use `xyz.genslate.slatecore.<app>`.
- Never reuse SlateSuite names or the previous owner or machine identifiers in tracked files.

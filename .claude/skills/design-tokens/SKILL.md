---
name: design-tokens
description: Change SLATECORE design tokens (colours, type, radius, layout sizes, motion) and regenerate the CSS, Tailwind theme, TypeScript, JSON and Rust outputs. Use when a token must be added or changed, or when token drift fails.
---

# Design tokens

`packages/tokens` is the single source for every visual value. It generates the CSS variables, the
Tailwind v4 theme, TypeScript, JSON and the Rust constants in `crates/design-tokens`. Generated
files are never edited by hand (a hook blocks it).

## Where things live

| Need | Edit |
|---|---|
| Palette and theme roles (surface, fg, border, accent, status) | `packages/tokens/src/themes/nord.polar-night.theme.ts`, `nord.snow-storm.theme.ts` |
| Colour tokens, cursor tokens | `packages/tokens/src/tokens/color.tokens.ts`, `cursor.tokens.ts` |
| Spacing, radius, z-index, layout sizes | `packages/tokens/src/tokens/layout.tokens.ts` |
| Type scale, fonts | `packages/tokens/src/tokens/typography.tokens.ts` |
| Durations, easing, distance, scale | `packages/tokens/src/tokens/motion.tokens.ts` |
| Key and type definitions | `token.keys.ts`, `token.types.ts` |
| Generators | `packages/tokens/scripts/build-tokens.ts` and `scripts/emit/` |

## Steps

1. Plan first if the change adds a colour, a theme role or a new token family beyond the approved
   plan (`.claude/rules/workflow.md`).
2. Add a failing test in `packages/tokens/tests/unit/` (a role exists, a value is an official Nord colour,
   contrast between a pair meets the threshold). Run `bun x --no-install turbo run test --filter=@genslate/tokens`.
3. Edit the source. Colours must be official Nord values (nord0 to nord15): Polar Night for dark
   surfaces, Snow Storm for light, Frost for accents, Aurora for status. Both themes get the new
   role. Keys are added in `token.keys.ts` first so the compiler lists what is missing.
4. Regenerate: `bun run tokens`. Commit the regenerated files with the source change.
5. Read `packages/tokens/src/generated/json/contrast-report.json`: every text pair must pass in
   both themes. Fix the token, not the threshold.
6. Verify drift is gone: `bun run tokens --check`, then `bun run check --ts`.
7. If the change is visible, review it in the Design Kit (`bun run dev --kit`) in both themes and
   commit fresh screenshots under `.claude/project/screenshots/design-kit/`.

## Rules

- No gradients, glows or inset shadows as tokens. The only shadow token is `shadow-popover`.
- Motion tokens collapse under reduced motion; do not add a duration that bypasses it.
- Never reference a hex value outside `packages/tokens/src`. Apps and components use utilities.
- Rust consumers (`genslate-design-tokens`) get the same values; the Tauri window background comes
  from there. Run `cargo test -p genslate-design-tokens --locked` after a change.

---
paths:
  - "packages/design-system/**"
  - "packages/tokens/**"
  - "programs/**/src/**"
---

# Design contract (flat Nord)

The Design Kit (`programs/webapp/example`, `bun run dev --kit`) is the living spec. Open the
section for the component you change before you edit it. Motion and states: `motion-polish.md`.

## Principles

- **Flat.** Chrome recedes. Surfaces separate by colour steps and 1px borders. No gradients,
  bevels, inset shadows or glows. The only shadow is `shadow-popover`, on floating menus,
  popovers, tooltips, toasts and dialogs.
- **Official Nord.** Dark is Polar Night (nord0-3), light is Snow Storm (nord4-6), accent is
  Frost (nord7-10), status colours are Aurora (nord11-15) only. Nothing else is a colour.
- **Tokens only.** Theme flips through `[data-theme]` (`polar-night`, `snow-storm`, `system`), so tokens
  change and classes do not: no `dark:` variants, no raw hex, rgb or oklch, no default Tailwind
  palette classes (the palette is removed from the theme).
- **A new colour is a token change** in `packages/tokens/src`, never an inline value. Both themes
  must pass the contrast report (`bun run tokens` writes
  `packages/tokens/src/generated/json/contrast-report.json`). Use the `design-tokens` skill.

## Token to utility

The generated `packages/tokens/src/generated/css/tailwind.theme.css` is the truth. Search it
instead of guessing a name.

| Need | Utilities |
|---|---|
| Surface | `bg-canvas`, `bg-surface`, `bg-surface-sidebar` `-panel` `-raised` `-popover` `-dialog` `-sunken`, `bg-field` |
| Text | `text-fg-strong`, `text-fg`, `text-fg-secondary`, `text-fg-muted`, `text-fg-disabled` |
| Border | `border-border-subtle`, `border-border`, `border-border-strong` |
| Accent | `bg-accent` (`-hover`, `-active`) with `text-on-accent`; `text-accent-fg`, `bg-accent-subtle`, `border-accent-border` |
| Status | `bg-danger` `-warning` `-success` `-info` with `text-on-*`; `text-*-fg`, `bg-*-subtle`, `border-*-border` |
| Controls | `bg-control` (`-hover`, `-pressed`), `bg-fill-hover`, `bg-fill-pressed`, `bg-selection` |
| Focus | `focus-ring`, `focus-ring-inset`: a crisp 2px Frost outline, never a blurred halo |
| Shape | `rounded-control` `-menu-item` `-popover` `-card` `-dialog` `-window`, or the scale `rounded-xs` to `rounded-2xl` |
| Size | `h-control-xs` to `h-control-xl`, `h-row-sm`, `h-row-md`, `max-w-dialog-md` |
| Type | `font-sans` (Inter), `font-mono` (JetBrains Mono), `text-xs` to `text-xl` (line height is paired) |
| Layer | `z-base` `-raised` `-sticky` `-chrome` `-popover` `-scrim` `-dialog` `-toast` `-tooltip` |
| Motion | `duration-instant` `-fast` `-base` `-slow`, `ease-standard` `-emphasized` `-decelerate`, `motion-*` |

Spacing is Tailwind's 4px grid. Do not add arbitrary pixel values when a token exists.

## Component contract

Components live in `packages/design-system/src/components/<category>/<name>/`. Categories:
`actions`, `display`, `feedback`, `inputs`, `layout`, `navigation`, `overlays`, `window`.

- **Files:** `<name>.component.tsx`, `<name>.types.ts`, `<name>.variants.ts` (Tailwind Variants
  `tv` from `utils/cn.util`), `index.ts`. Each category has an `index.ts` barrel, and the package
  `exports` map exposes `@genslate/design-system/<category>`.
- **Code:** function components, named exports, `ref` as a prop (no `forwardRef`), `import type`.
  Base UI primitives (`@base-ui/react`) for behaviour and accessibility. Composition uses the
  Base UI `render` prop, never `asChild`. Every part carries `data-slot`. States are expressed
  with data attributes (`data-disabled`, `data-open`, `data-loading`, `window-inactive:`).
- **Pure UI:** the design system is Tauri-free (no `@tauri-apps/*`, no IPC, no `window.__TAURI__`),
  imports no CSS from components, and has no side effects at import time.
- **Text:** no hard-coded user-facing strings in a component. Strings come through a `labels`
  prop with an English default, so apps can translate.
- **Accessibility:** WCAG 2.2 AA. Keyboard operable, visible focus, correct roles and names, a
  hit area of at least 24px, `aria-busy` while loading, no colour-only meaning. Icon-only controls
  need a label.
- **Icons:** one set, Codicons (`@vscode/codicons`, names generated into
  `src/icons/codicon-names.generated.ts`), at the fixed sizes of `IconSize` (12, 14, 16, 20).
- **Tests:** `packages/design-system/tests/unit/<category>/<name>.test.tsx` with bun test and
  happy-dom. Cover every variant, state, keyboard path and `labels`. Run
  `bun x --no-install turbo run test typecheck --filter=@genslate/design-system`.
- **Showcase:** every component has a page in the Design Kit
  (`programs/webapp/example/src/features/showcase/sections/<category>/`) with a props table, a
  state matrix and, for overlays, an open specimen. Use the `new-component` skill.

## App code (`programs/**/src`)

Apps compose design-system components and token utilities; they do not restyle them. Layout and
feature code uses the same tokens. IPC goes through `@genslate/tauri-bridge`, which falls back to
a mock in the browser. User-visible strings follow `branding.md`.

## Gate

Nothing is done until the Design Kit shows it in both themes with every state and its motion, the
`ui-visual-qa` agent has passed it, and the screenshots are committed under
`.claude/project/screenshots/<app>/` (see `workflow.md`).

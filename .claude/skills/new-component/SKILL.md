---
name: new-component
description: Add a component to the SLATECORE design system (packages/design-system) with types, variants, tests and a Design Kit page. Use when asked to create or extend a design-system component, or run as /new-component <category>/<name>.
argument-hint: <category>/<name>
---

# New design-system component

Target: `$ARGUMENTS` (for example `actions/chip`). Categories: `actions`, `display`, `feedback`,
`inputs`, `layout`, `navigation`, `overlays`, `window`.

## 0. Before you start

- A component beyond the approved launcher plan needs the owner's approval first
  (`.claude/rules/workflow.md`): post the plan, wait.
- Read `.claude/rules/design-contract.md` and `.claude/rules/motion-polish.md`, the closest
  sibling in `packages/design-system/src/components/<category>/`, and the Base UI docs for the
  primitive you build on (Context7 or base-ui.com; do not guess its API).
- Check the design system does not already have it: search the barrels and the Design Kit.

## 1. Test first

Create `packages/design-system/tests/unit/<category>/<name>.test.tsx` (bun test, happy-dom).
Cover every variant and size, each state (rest, hover class, pressed, focus-visible, selected,
disabled, invalid or loading where they apply), keyboard operation, accessible name and role,
`labels` overrides, `data-slot` on each part, and the `className` pass-through. Run it and watch
it fail:

```
bun x --no-install turbo run test --filter=@genslate/design-system
```

## 2. Files

In `packages/design-system/src/components/<category>/<name>/`:

| File | Holds |
|---|---|
| `<name>.types.ts` | Props (extend the Base UI props), variant unions, a `labels` type with English defaults |
| `<name>.variants.ts` | `tv({ slots, variants, defaultVariants })` from `../../../utils/cn.util` |
| `<name>.component.tsx` | The function component, `ref` as a prop, `data-slot` on every part |
| `index.ts` | Named exports of the component, its types and the variants |

Then add `export * from './<name>';` to the category `index.ts`. The package `exports` map already
exposes `@genslate/design-system/<category>` and the root barrel re-exports every category.

Rules that bite:

- Token utilities only (see the table in `design-contract.md`): no hex, no `dark:`, no palette
  classes, no inline pixel values where a token exists.
- State through Base UI data attributes (`data-disabled`, `data-open`, `data-checked`) and
  `not-data-disabled:` guards on hover and active styles.
- Focus uses `focus-ring` or `focus-ring-inset`. Motion uses `duration-*` and `ease-*` tokens and
  the `motion-*` utilities; transform and opacity only.
- Compose with the Base UI `render` prop, never `asChild`; no `forwardRef`; no default export; no
  `@tauri-apps/*`; no CSS import; no user-facing string literals outside `labels`.
- Reuse recipes (`src/recipes/`) for fields, list items and popup surfaces.
- Icons: `renderIconSlot` with Codicon names at sizes 12, 14, 16 or 20.

## 3. Design Kit page (required)

In `programs/webapp/example/src/features/showcase/sections/<category>/`:

1. `<name>.section.tsx` with `Specimen` blocks (variants, sizes, composition), a `StateMatrix`
   (every state, hover and pressed forced with the same classes the component uses) and a
   `PropsTable`. Overlays also show an open specimen.
2. Register it in `<category>.sections.ts` with `id`, `title`, `description`, `category`, `icon`
   and `covers: ['<ExportedName>']`. A test fails when an exported component is covered by no page.
3. Update the kit tests if the page count is asserted.

## 4. Verify

```
bun x --no-install turbo run test typecheck --filter=@genslate/design-system --filter=@genslate/example
bun run check --ts
```

Then run the Design Kit (`bun run dev --kit`, port 1430) and check the page in both themes:
every state, keyboard focus, reduced motion (see the review method in `motion-polish.md`), and
both light and dark contrast. Run the `ui-visual-qa` agent on the page.

## 5. Finish

- Screenshots of the new page in both themes into `.claude/project/screenshots/design-kit/` as
  `design-kit-<polar-night|snow-storm>-<category>-<name>.png` (plus `-open` variants), committed
  with the work and shown in your reply.
- A `.changes/unreleased/` fragment only if the change is user-visible (`/change`).
- Commit with scope `design-system` (or `example` for kit-only changes), staging only your paths.

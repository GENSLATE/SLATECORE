# SLATECORE Design Kit

`@genslate/example`: every `@genslate/design-system` component in both Nord themes (Polar Night, Snow Storm), with states, foundations (colors, typography, spacing, motion, icons, cursors) and the provider stack. It is the visual review gate for the design system and runs as a plain web app on a local bun dev server. No Tauri, no native shell.

## Run it

From the repository root:

```sh
bun run dev --kit
```

or directly:

```sh
bun run --cwd programs/webapp/example dev
```

The Kit is served at <http://localhost:1430> (`strictPort`: if the port is busy the server stops instead of moving).

| Script | What it does |
| --- | --- |
| `dev` | Vite dev server on port 1430 |
| `build` | Production build into `dist/` |
| `preview` | Serves the production build on port 1430 |
| `typecheck` | `tsc --noEmit` |
| `test` | `bun test` with a happy-dom preload |

## Using the Kit

- The title bar toggle (or `Ctrl/⌘ + Shift + L`) switches Polar Night and Snow Storm at runtime; the status bar and the Appearance inspector (gear) also set Dark, Light or System.
- `Ctrl/⌘ + K` opens the command palette to jump to any page, `Ctrl/⌘ + B` toggles the sidebar.
- The Motion page demonstrates `motion-fade-up`, `motion-row-in`, `motion-pop` and `motion-pulse-soft`, with a reduced-motion switch.
- Hover, pressed and focus columns in the state matrices are forced with classes so every state is visible at once.

## Adding a page

1. Write `src/features/showcase/sections/<category>/<name>.section.tsx` (use `Specimen`, `StateMatrix` and `PropsTable`).
2. Register it in that category's `*.sections.ts`, listing the design-system components it shows in `covers`.
3. `tests/unit/showcase.test.tsx` fails when the design system exports a component that no page covers, so a new component cannot ship without a page.

## Layout

```
src/
  app/        providers, shell, persisted state, Kit name
  features/   title bar, sidebar, status bar, command palette, inspector, context menu
  features/showcase/   registry, specimen components and one page per component or foundation
tests/unit/   app, showcase registry and coverage, motion, password field
```

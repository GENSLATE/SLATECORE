---
paths:
  - "packages/design-system/**"
  - "packages/tokens/**"
  - "programs/**/src/**"
---

# Motion and polish

Motion and polish are requirements, not extras. The motion layer lives in
`packages/design-system/src/styles/design-system.animations.css`; durations and curves are
tokens in `packages/tokens/src/tokens/motion.tokens.ts`.

## Rules

- **Tokens only.** Use `duration-instant|fast|base|slow` and `ease-standard|emphasized|decelerate`
  (`--gs-duration-*`, `--gs-ease-*`). Components never hard-code milliseconds or curves.
- **Reduced motion is handled once,** at token level: under `prefers-reduced-motion: reduce` every
  duration collapses and the enter distance and scale are neutralised. JS-driven animation reads
  `useReducedMotion` from the design system and honours it too.
- **Animate `transform` and `opacity`** (and `clip-path` for reveals). Never animate width, height,
  top or left in the webview. The Tauri window cannot move and resize atomically, so reveal
  content inside a fixed-size frame instead of resizing the window.
- **Signature animations,** all required: widen and shrink with a content cross-fade, a sliding tab
  indicator, staggered row entrance (`motion-row-in` with `--stagger`), launch pop (`motion-pop`),
  fade-and-scale for menus and toasts, a soft pulse on Coming Soon teasers (`motion-pulse-soft`).
  Content entering a view uses `motion-fade-up`.
- **Every control has every state:** rest, hover, pressed, focus-visible, selected, disabled, and
  invalid or loading where they apply. One focus ring everywhere (`focus-ring`). Hover and press
  move one colour step, they do not change layout.
- **Designed loading, empty and error states.** No blank panels, no raw error text, no layout jump
  when content arrives (skeletons keep the final size).
- **Calm, not busy.** One animated thing at a time per region; nothing loops except spinners,
  indeterminate bars and the teaser pulse.

## Review method

Screenshots alone cannot show motion, so the Design Kit review includes:

1. Emulate reduced motion and confirm nothing moves and nothing breaks.
2. Pause and sample with `document.getAnimations()` at 0, 50 and 100 percent of each animation.
3. Record pass or fail per animation and per state in the `ui-visual-qa` report.

Then capture the screenshots in both themes as described in `workflow.md`.

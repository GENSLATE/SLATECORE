---
name: ui-fixes
description: Find and fix a visual defect in SLATECORE UI in both themes (Polar Night and Snow Storm), then capture screenshots. Use for layout, colour, spacing, state or motion bugs in the Design Kit or the launcher.
---

# UI fixes

1. **Plan first.** Describe the defect and the proposed fix and wait for the owner's approval
   unless the fix is inside an approved task (`.claude/rules/workflow.md`).
2. **Reproduce** in a browser: `bun run dev --kit` (Design Kit, port 1430) or `bun run dev --web`
   (launcher mock, port 1420). Look at it in both themes by setting `[data-theme]` to `polar-night`
   and `snow-storm`. Note the view, theme, state and size.
3. **Read the contract:** `.claude/rules/design-contract.md` and `.claude/rules/motion-polish.md`.
   The defect is a deviation from it (a raw colour, a missing state, a hard-coded duration) or a
   gap in it. Say which.
4. **Test first** where happy-dom can see it (classes, attributes, structure): add a failing
   test in the package's `tests/` folder. Layout needs a real browser; measure it there.
5. **Fix at the source:** token utilities only, no hex, no `dark:`, no gradients, bevels or inset
   shadows; change the design-system component rather than patching an app. Tokens come from
   `packages/tokens/src` and `bun run tokens`; never edit generated files.
6. **Verify** both themes, every state, keyboard focus, and reduced motion. Run
   `bun x --no-install turbo run test typecheck --filter=@genslate/<package>` and `bun run check --ts`.
7. **Screenshots:** capture every changed view in both themes as
   `<app>-<polar-night|snow-storm>-<view>.png` into `.claude/project/screenshots/<app>/` with
   the same framing as the existing shots (Design Kit 1440 by 900). Replace only the pairs you
   changed, commit them with the fix and show them in your reply.
8. Report the defect, the cause, the change and the evidence (before and after).

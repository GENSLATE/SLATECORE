# Owner workflow (standing rules)

Two standing rules from the owner. They apply to every agent and every task, and they are always
on (this file has no `paths`).

## 1. Plan first

Make a plan and get the owner's approval before doing any work.

- The launcher plan (`docs/superpowers/plans/2026-10-07-genslate-launcher-plan.md`) is approved:
  work on its tasks as written.
- Anything beyond it is not: new features, scope changes, new apps, new dependencies, a bug fix
  that grows past its task. Post the plan and wait for the owner's approval before building.
  Approving one plan does not approve the next, and silence is not approval.
- A plan says what you will do and why, which paths you will touch, the risks, how you will
  verify it (tests, gates, screenshots) and what is out of scope.
- Claude Code: use plan mode and ask for approval there. Cursor and Antigravity: post the plan in
  the chat and stop until the owner replies.

## 2. Screenshots

When a piece of UI work is done, screenshot every feature and view you built or changed, in both
themes (Polar Night and Snow Storm), before you report it as done.

- **Name:** `<app>-<polar-night|snow-storm>-<view>.png`, lowercase kebab-case, for example
  `launcher-polar-night-settings-about.png` and `launcher-snow-storm-settings-about.png`.
  `<app>` is the app folder name (`launcher`, `design-kit`), `<view>` says what is shown
  (`settings-vault-unlocked`, `overlays-dialog-open`, `empty-state`).
- **Where:** `.claude/project/screenshots/<app>/`, for example
  `.claude/project/screenshots/launcher/`. Commit them with the work, in the same commit.
- **Both themes, always.** A view without its twin fails `bun run check`. Menus, dialogs,
  tooltips, empty, error, loading and teaser states count as views of their own.
- **Consistent framing,** the same for every shot of one app: fixed viewport (the Design Kit at
  1440 by 900), device scale 1, cursor parked away from the content, seed data, no dev overlays,
  nothing personal or machine-specific on screen. Set the theme through `[data-theme]` rather than
  the OS, so both shots show the same state.
- **Capture** from the Design Kit (`bun run dev --kit`) or the launcher browser mock
  (`bun run dev --web`) with a browser tool such as Playwright.
- **Show them in the chat reply,** one image per view and theme (or a contact sheet), with their
  paths. They are reused for READMEs and websites, so they must be presentable.
- **Do not delete or alter** screenshots you did not take or views you did not change. When you
  change a view, retake both themes and replace that pair.

---
trigger: always_on
description: Owner workflow rules for every task (plan first, screenshots in both themes)
---
Follow `AGENTS.md` and `.claude/rules/workflow.md` (read the second file before you start).
@../../.claude/rules/workflow.md
1. Plan first: make a plan and get the owner's approval before doing any work. Beyond the
   approved launcher plan, post the plan and wait for approval before building. Diagnose, propose
   the change, then fix when asked.
2. Screenshots: when UI work is done, capture every view in Polar Night and Snow Storm as
   `<app>-<polar-night|snow-storm>-<view>.png` in `.claude/project/screenshots/<app>/`,
   commit them with the work and show them in the chat reply.

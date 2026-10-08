---
name: release
description: Prepare a SLATECORE LAUNCHER release (preflight gates, version bump, changelog, portable zip, portability test) and ask before tagging. Use only when the owner asks for a release.
disable-model-invocation: true
argument-hint: <patch|minor|major|x.y.z>
---

# Release

A release is irreversible once tagged: do every step in order, report each result, and ask the
owner before the tag and any push. Nothing here runs without the owner's go-ahead.

## 1. Preflight

1. Clean tree on the release branch: `git status`, `git log origin/main..HEAD --oneline`.
2. `bun run check` and `bun run test` are green. Fix failures; do not skip hooks.
3. The portability test passes (`.claude/rules/portability-no-trace.md`). A release with a new
   file, folder, environment variable or registry access needs the test extended first.
4. `bun run attribution --range origin/main..HEAD --current-branch` passes (no agent credit).
5. UI changed since the last release: screenshots in both themes are current in
   `.claude/project/screenshots/<app>/` (`.claude/rules/workflow.md`).
6. Run the `security-reviewer` agent on the diff since the last tag.

## 2. Version and changelog

1. `bun run version $ARGUMENTS --dry-run`, show the plan, then `bun run version $ARGUMENTS`. It bumps
   every `package.json`, `[workspace.package]` and literal `tauri.conf.json` versions, then updates
   both lockfiles.
2. Fold `.changes/unreleased/` into the release notes with changie
   (`.claude/rules/changes.md`); read the result, names in capitals (SLATECORE LAUNCHER, GENSLATE).
3. Commit: `chore(release): v<version>`, staging only the version files, lockfiles, `CHANGELOG.md`
   and `.changes/`.

## 3. Package

1. `bun run package` (asks for permission; older builds move to `release/.archive/`). It writes
   `release/slatecore-launcher-<version>-base.zip` and, when asked, the
   `...-webview2-runtime.zip`.
2. Unpack the zip into a scratch folder on another drive letter and start it there. It must run
   from a path with spaces, write nothing outside the folder, and the vault must unlock.
3. List what the zip holds and check it contains no `.env`, no vault data and no absolute paths.

## 4. Ask, then tag

Show the owner: version, changelog, zip names and sizes, gate results. After an explicit yes:
`git tag v<version>` and `git push` of the branch and tag (both ask for permission). The tag starts
`.github/workflows/release.yml`, which reuses CI and publishes. Never force-push, never retag.

## Do not

- Publish crates (`cargo publish` is denied), upload anywhere else, or write release notes by hand.
- Tag with failing gates or an unreviewed security diff.

---
description: Run every SLATECORE gate and fix what fails (add --no-fix to only report)
argument-hint: "[--no-fix] [--ts|--rust]"
allowed-tools: Bash(bun run *), Bash(bun x --no-install *), Bash(cargo fmt *), Bash(cargo clippy *), Bash(cargo test *), Bash(cargo deny *), Bash(cargo machete *), Read, Edit, Grep, Glob
---

Run the repository gates and report. Arguments: $ARGUMENTS

1. Run `bun run check`, passing `--ts` or `--rust` through when given. It covers the lockfile,
   typecheck, Biome, cspell, knip, the Rust gates and the agent-folder check.
2. Unless `--no-fix` was given, fix each failure at its cause: formatting with `bun run format`,
   token drift with `bun run tokens`, a real typo in the text, a legitimate new word in
   `.config/cspell/project-words.txt`. Never silence a rule, loosen a config or edit a generated
   file.
3. Rerun until it is green, or until a failure needs a decision from the owner (say which).
4. Report the gates that ran, each failure and its fix (files), and anything still red and why.
   Failures in files you did not touch belong to another agent's work in the tree: report them,
   do not fix them. Run `bun run test` only when asked.

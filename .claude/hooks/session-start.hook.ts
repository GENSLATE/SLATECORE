/**
 * SessionStart: gives Claude a short repository snapshot (branch, uncommitted files, toolchain,
 * commands) and keeps the git identity human. Cloud containers commit as the agent by default,
 * which the authorship check rejects, so an agent-looking `user.name`/`user.email` in this clone
 * is replaced by the last human author found in the history.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { checkBranch, checkIdentity, LOG_FORMAT, parseLog } from '../../scripts/lib/attribution';
import { addContext, projectDir, readInput, run } from './hook.shared';

function git(root: string, args: readonly string[]): string {
  try {
    const result = Bun.spawnSync(['git', ...args], { cwd: root, stdout: 'pipe', stderr: 'pipe' });
    return result.exitCode === 0 ? result.stdout.toString().trim() : '';
  } catch {
    return '';
  }
}

function read(root: string, path: string): string {
  try {
    return readFileSync(join(root, path), 'utf8');
  } catch {
    return '';
  }
}

/** `bun x.y.z, rust x.y.z` from `package.json` and `rust-toolchain.toml` (never hard-coded here). */
function toolchain(root: string): string {
  const bun = /"packageManager"\s*:\s*"bun@([^"]+)"/.exec(read(root, 'package.json'))?.[1];
  const rust = /channel\s*=\s*"([^"]+)"/.exec(read(root, 'rust-toolchain.toml'))?.[1];
  return [bun !== undefined ? `bun ${bun}` : '', rust !== undefined ? `rust ${rust}` : '']
    .filter(Boolean)
    .join(', ');
}

/** Replaces an agent git identity with the last human author. Returns a note for Claude. */
function fixIdentity(root: string): string | undefined {
  const name = git(root, ['config', 'user.name']);
  const email = git(root, ['config', 'user.email']);
  if (name === '' && email === '') return undefined;
  if (checkIdentity('git config', 'author', name, email).length === 0) return undefined;

  const human = parseLog(git(root, ['log', '-n', '200', `--format=${LOG_FORMAT}`])).find(
    (commit) =>
      commit.authorName !== '' &&
      checkIdentity('log', 'author', commit.authorName, commit.authorEmail).length === 0,
  );
  if (human === undefined) {
    return `The git identity ${name} <${email}> names an agent and no human author was found: set user.name and user.email before committing.`;
  }
  git(root, ['config', '--local', 'user.name', human.authorName]);
  git(root, ['config', '--local', 'user.email', human.authorEmail]);
  return `The git identity ${name} <${email}> named an agent; this clone now commits as ${human.authorName} (the last human author).`;
}

if (import.meta.main) {
  await run(async () => {
    const input = await readInput();
    const root = projectDir();
    const branch = git(root, ['branch', '--show-current']);
    const dirty = git(root, ['status', '--porcelain']).split('\n').filter(Boolean).length;

    const lines = [
      'SLATECORE agent session',
      `Branch ${branch === '' ? '(detached or no git)' : branch}, ${dirty} uncommitted file${dirty === 1 ? '' : 's'}. ${toolchain(root)}`.trim(),
      'Commands: bun run check | test | format | tokens | dev [--web|--kit] | new-app <name> | attribution.',
      'Rules: AGENTS.md, plus .claude/rules/*.md when you touch their paths. Make a plan and get the owner approval before building; screenshot finished UI in both themes into .claude/project/screenshots/<app>/.',
    ];
    const identity = fixIdentity(root);
    if (identity !== undefined) lines.push(identity);
    if (checkBranch(branch).length > 0) {
      lines.push(`The branch name ${branch} names an agent; rename it before pushing.`);
    }
    addContext(input.eventName ?? 'SessionStart', lines.join('\n'));
  });
}

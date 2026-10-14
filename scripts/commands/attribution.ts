/**
 * `bun run attribution`: rejects commits, branches and PR bodies that credit an agent as author
 * (agent git identity, Co-Authored-By and session trailers, session links, "Generated with"
 * footers, agent branch prefixes). Run by lefthook (commit-msg, pre-push) and CI.
 */
import { readFileSync, writeFileSync } from 'node:fs';

import { defineCommand } from '../lib/args';
import {
  type AttributionFinding,
  type CommitIdentity,
  checkBranch,
  checkCommit,
  checkIdentity,
  checkText,
  LOG_FORMAT,
  parseIdent,
  parseLog,
  stripAgentCredit,
} from '../lib/attribution';
import { log } from '../lib/log';
import { capture } from '../lib/run';

async function git(args: readonly string[]): Promise<string> {
  const result = await capture(['git', ...args]);
  if (result.code !== 0) throw new Error(`git ${args.join(' ')} failed: ${result.stderr.trim()}`);
  return result.stdout;
}

/** The identity git will commit as; `undefined` when git has none (git then refuses to commit). */
async function identity(variable: string): Promise<{ name: string; email: string } | undefined> {
  const result = await capture(['git', 'var', variable]);
  return result.code === 0 ? parseIdent(result.stdout) : undefined;
}

/**
 * Commits in `range`. When the base ref is missing (a new clone or a remote with no main yet) it
 * falls back to the commits that are on no remote, so a first push is still checked.
 */
async function commitsInRange(range: string): Promise<CommitIdentity[]> {
  const format = `--format=${LOG_FORMAT}`;
  const direct = await capture(['git', 'log', format, range]);
  if (direct.code === 0) return parseLog(direct.stdout);
  log.warn(`${range} is not available; checking the commits that are on no remote`);
  return parseLog(await git(['log', format, 'HEAD', '--not', '--remotes']));
}

async function checkPendingCommit(messageFile: string): Promise<AttributionFinding[]> {
  const author = await identity('GIT_AUTHOR_IDENT');
  const committer = await identity('GIT_COMMITTER_IDENT');
  return [
    ...(author === undefined
      ? []
      : checkIdentity('new commit', 'author', author.name, author.email)),
    ...(committer === undefined
      ? []
      : checkIdentity('new commit', 'committer', committer.name, committer.email)),
    ...checkText('new commit', readFileSync(messageFile, 'utf8')),
  ];
}

await defineCommand({
  name: 'attribution',
  summary: 'Reject commits, branches and PR bodies that credit an agent as author.',
  usage:
    '[--message <file> [--fix]] [--range <base..head>] [--branch <name> | --current-branch] [--body-env <VAR>]',
  options: {
    message: {
      type: 'string',
      description: 'Check a commit message file and the identity git will commit as (commit-msg).',
    },
    fix: {
      type: 'boolean',
      description: 'With --message: strip agent trailers and footers from the file, then check.',
    },
    range: {
      type: 'string',
      description: 'Check every commit in a git range, e.g. origin/main..HEAD (pre-push, CI).',
    },
    branch: { type: 'string', description: 'Check a branch name.' },
    'current-branch': { type: 'boolean', description: 'Check the checked-out branch name.' },
    'body-env': {
      type: 'string',
      description: 'Check the text in this environment variable (a PR body in CI).',
    },
  },
  details:
    'Using agent tooling is fine; crediting it is not. Plain mentions of .claude/ or CLAUDE.md pass.\n' +
    'Fix a rejected commit with `git commit --amend` (or `git rebase` for older ones) and set\n' +
    '`git config user.name` / `user.email` to your own identity.',
  async run({ values }) {
    const findings: AttributionFinding[] = [];
    if (values.message !== undefined) {
      if (values.fix) {
        const before = readFileSync(values.message, 'utf8');
        const after = stripAgentCredit(before);
        if (after !== before) {
          writeFileSync(values.message, after);
          log.info(`removed agent credit from ${values.message}`);
        }
      }
      findings.push(...(await checkPendingCommit(values.message)));
    }
    if (values.range !== undefined) {
      findings.push(...(await commitsInRange(values.range)).flatMap(checkCommit));
    }
    const branch = values['current-branch']
      ? (await git(['branch', '--show-current'])).trim()
      : values.branch;
    if (branch !== undefined && branch !== '') findings.push(...checkBranch(branch));
    const bodyEnv = values['body-env'];
    if (bodyEnv !== undefined) findings.push(...checkText('PR body', process.env[bodyEnv] ?? ''));

    if (findings.length === 0) {
      log.success('no agent attribution found');
      return;
    }
    for (const { where, reason } of findings) log.error(`${where}: ${reason}`);
    log.error('SLATECORE commits, branches and PRs must not credit an agent as author.');
    process.exitCode = 1;
  },
});

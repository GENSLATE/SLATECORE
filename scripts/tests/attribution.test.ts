import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  AGENT_BRANCH_PREFIXES,
  AGENT_NAMES,
  checkBranch,
  checkCommit,
  checkIdentity,
  checkText,
  parseIdent,
  parseLog,
  stripAgentCredit,
} from '../lib/attribution';
import { ROOT } from '../lib/paths';

const OWNER = { name: 'GENSLATE Dev', email: 'dev@example.com' };

const reasons = (text: string): string[] => checkText('m', text).map(({ reason }) => reason);

describe('commit message trailers and footers', () => {
  test('normal_message_passes', () => {
    expect(checkText('m', 'feat(scripts): add a tray menu\n\nBody.')).toEqual([]);
  });

  test('plain_tooling_mentions_pass', () => {
    expect(
      checkText('m', 'chore(agents): update .claude/rules, .cursor/rules and CLAUDE.md'),
    ).toEqual([]);
    expect(checkText('m', 'docs: explain how Cursory reads work')).toEqual([]);
  });

  test('human_coauthor_passes', () => {
    expect(checkText('m', 'fix: x\n\nCo-authored-by: Jo Writer <jo.writer@gmail.com>')).toEqual([]);
    expect(checkText('m', 'fix: x\n\nco-authored-by: Sam <sam@example.com>')).toEqual([]);
  });

  for (const name of AGENT_NAMES) {
    test(`rejects_coauthor_trailer_naming_${name}`, () => {
      const upper = name.toUpperCase();
      expect(reasons(`fix: x\n\nCo-Authored-By: ${upper} <noreply@${name}.com>`)).toHaveLength(1);
      expect(
        reasons(`fix: x\n\nco-authored-by: ${name} Bot <bot@users.noreply.github.com>`),
      ).toHaveLength(1);
    });

    test(`rejects_agent_keyed_trailer_naming_${name}`, () => {
      expect(reasons(`fix: x\n\n${name}-Session: abc123`)).toHaveLength(1);
    });

    test(`rejects_generated_with_footer_naming_${name}`, () => {
      expect(reasons(`fix: x\n\n🤖 Generated with [${name}](https://example.com)`)).toHaveLength(1);
      expect(reasons(`fix: x\n\nMade with ${name}`)).toHaveLength(1);
    });
  }

  test('rejects_agent_session_links', () => {
    for (const link of [
      'https://claude.ai/code/session_01abc',
      'https://cursor.com/agents/bc-123',
      'https://app.devin.ai/sessions/abc',
      'https://chatgpt.com/codex/tasks/task_1',
      'https://jules.google/task/1',
    ]) {
      expect(reasons(`Body\n\n${link}`)).toEqual(['an agent session link']);
    }
  });

  test('git_comment_lines_are_ignored', () => {
    expect(checkText('m', 'fix: x\n\n# Co-authored-by: Claude <noreply@anthropic.com>\n')).toEqual(
      [],
    );
  });
});

describe('identities and branches', () => {
  test('rejects_agent_identities_and_passes_the_owner', () => {
    expect(checkIdentity('c', 'author', OWNER.name, OWNER.email)).toEqual([]);
    expect(checkIdentity('c', 'author', 'Claude', 'noreply@anthropic.com')).toHaveLength(1);
    expect(checkIdentity('c', 'committer', 'Bot', 'bot@anthropic.com')).toHaveLength(1);
    expect(checkIdentity('c', 'author', 'Cursor Agent', 'cursoragent@cursor.com')).toHaveLength(1);
    expect(
      checkIdentity('c', 'author', 'renovate[bot]', 'renovate@whitesourcesoftware.com'),
    ).toEqual([]);
  });

  for (const prefix of AGENT_BRANCH_PREFIXES) {
    test(`rejects_branch_prefix_${prefix.replace('/', '')}`, () => {
      expect(checkBranch(`${prefix}kind-name-j4bg82`)).toHaveLength(1);
      expect(checkBranch(`${prefix.toUpperCase()}x`)).toHaveLength(1);
    });
  }

  test('passes_ordinary_branches', () => {
    for (const name of ['main', 'build/launcher-v1', 'feat/tray-menu', 'fix/claude-md-typo']) {
      expect(checkBranch(name)).toEqual([]);
    }
  });
});

describe('log parsing', () => {
  test('flags_only_the_agent_commit', () => {
    const record = (fields: readonly string[]) => `${fields.join('\u001F')}\u001E\n`;
    const output =
      record(['a'.repeat(40), OWNER.name, OWNER.email, OWNER.name, OWNER.email, 'feat: a\n']) +
      record([
        'b'.repeat(40),
        'Claude',
        'noreply@anthropic.com',
        'Claude',
        'noreply@anthropic.com',
        'feat: b\n\nClaude-Session: https://claude.ai/code/session_x\n',
      ]);
    const [owner, agent, ...rest] = parseLog(output);
    expect(rest).toEqual([]);
    expect(owner && checkCommit(owner)).toEqual([]);
    // Author, committer and the session trailer line.
    expect(agent && checkCommit(agent)).toHaveLength(3);
  });

  test('parseIdent_splits_git_var_output', () => {
    expect(parseIdent(`${OWNER.name} <${OWNER.email}> 1700000000 +0000`)).toEqual(OWNER);
  });
});

describe('stripAgentCredit', () => {
  test('keeps_a_clean_message_byte_for_byte', () => {
    const clean = 'feat: a\n\nBody with details.\n\nCo-authored-by: Jo <jo@gmail.com>\n';
    expect(stripAgentCredit(clean)).toBe(clean);
  });

  test('removes_agent_lines_and_the_blank_lines_they_leave', () => {
    const dirty =
      'feat: a\n\nBody.\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n🤖 Generated with [Claude Code](https://claude.com/claude-code)\n';
    expect(stripAgentCredit(dirty)).toBe('feat: a\n\nBody.\n');
  });
});

describe('bun run attribution', () => {
  let dir: string | undefined;

  afterEach(async () => {
    if (dir !== undefined) await rm(dir, { recursive: true, force: true });
    dir = undefined;
  });

  /** Runs the command with a fixed human git identity. */
  async function attribution(...args: string[]): Promise<{ code: number; output: string }> {
    const child = Bun.spawn([process.execPath, 'scripts/commands/attribution.ts', ...args], {
      cwd: ROOT,
      stdout: 'pipe',
      stderr: 'pipe',
      env: {
        ...process.env,
        NO_COLOR: '1',
        GIT_AUTHOR_NAME: OWNER.name,
        GIT_AUTHOR_EMAIL: OWNER.email,
        GIT_COMMITTER_NAME: OWNER.name,
        GIT_COMMITTER_EMAIL: OWNER.email,
      },
    });
    const [out, err, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    return { code, output: out + err };
  }

  async function messageFile(text: string): Promise<string> {
    dir = await mkdtemp(join(tmpdir(), 'slatecore-attribution-'));
    const file = join(dir, 'COMMIT_EDITMSG');
    await writeFile(file, text);
    return file;
  }

  test('message_without_trailers_passes', async () => {
    const file = await messageFile('feat(scripts): add things\n\nPlain body.\n');
    const result = await attribution('--message', file);
    expect(result.code).toBe(0);
  });

  test('message_with_agent_trailer_fails_and_names_the_reason', async () => {
    const file = await messageFile('fix: x\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n');
    const result = await attribution('--message', file);
    expect(result.code).toBe(1);
    expect(result.output).toContain('credits an agent');
  });

  test('fix_strips_known_trailers_from_message_file', async () => {
    const file = await messageFile(
      'fix: x\n\nBody.\n\nCo-Authored-By: Claude <noreply@anthropic.com>\nGenerated with Cursor\n',
    );
    const result = await attribution('--message', file, '--fix');
    expect(result.code).toBe(0);
    expect(await readFile(file, 'utf8')).toBe('fix: x\n\nBody.\n');
  });

  test('agent_branch_name_fails', async () => {
    expect((await attribution('--branch', 'claude/kind-name')).code).toBe(1);
    expect((await attribution('--branch', 'build/launcher-v1')).code).toBe(0);
  });
});

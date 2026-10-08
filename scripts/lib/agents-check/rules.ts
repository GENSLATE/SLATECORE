/** Claude rules (`paths:`), Cursor and Antigravity shims, skills, commands and agents. */
import { dirname, join, relative, resolve } from 'node:path';

import {
  type Context,
  checkLines,
  exists,
  frontmatter,
  list,
  nonEmptyString,
  patternsOf,
  problem,
  read,
} from './context';

/** The owner's standing workflow rules: loaded in every session, so never path-scoped. */
const WORKFLOW = 'workflow';

/** Cursor rules without `globs` that the agent requests by description. */
const AGENT_REQUESTED = ['changes'];

/** Antigravity `trigger` values; a rule without one of them is silently dropped. */
const TRIGGERS = ['always_on', 'model_decision', 'glob', 'manual'];

const ANTIGRAVITY_CHARS = 12_000; // The tool's limit per rule file.
const SHIM_LINES = 15; // Pointers, not prose.
const SKILL_LINES = 500; // Claude's own guidance for SKILL.md.

async function checkClaudeRules(ctx: Context): Promise<void> {
  for (const name of await list(ctx, '.claude/rules')) {
    if (name.startsWith('.')) continue;
    const rel = `.claude/rules/${name}`;
    if (!name.endsWith('.md')) {
      problem(ctx, rel, 'Claude Code loads only .md files from .claude/rules; rename it to .md');
      continue;
    }
    const text = await read(ctx, rel);
    if (text === undefined) continue;
    const { data, error } = frontmatter(text);
    if (error !== undefined) problem(ctx, rel, error);
    if (name === `${WORKFLOW}.md`) {
      if (data !== undefined && 'paths' in data) {
        problem(ctx, rel, 'must not have paths: (the owner workflow rules are always on)');
      }
    } else if (patternsOf(data?.['paths']).length === 0) {
      problem(ctx, rel, 'needs paths: (a list of globs), or it loads into every session');
    }
  }
}

/** Every file a shim points at: `@path` references and backticked repository paths. */
function pointersOf(body: string): { readonly token: string; readonly at: boolean }[] {
  const pointers: { token: string; at: boolean }[] = [];
  for (const match of body.matchAll(/(?:^|\s)@([^\s`]+)/g)) {
    pointers.push({ token: (match[1] ?? '').replace(/[.,;:)]+$/, ''), at: true });
  }
  for (const match of body.matchAll(/`([^`\n]+)`/g)) {
    const token = match[1] ?? '';
    const file = /^(?:\.claude|\.cursor|\.agents)\/[^*\s<>]+\.[a-z]+$/.test(token);
    if (file || /^(?:AGENTS|CLAUDE)\.md$/.test(token)) pointers.push({ token, at: false });
  }
  return pointers;
}

/** Every shim names at least one canonical file, and every file it names exists. */
async function checkPointers(ctx: Context, rel: string, body: string): Promise<void> {
  // `@` references: Cursor resolves them from the project root, Antigravity from the rule file.
  const atBase = rel.startsWith('.agents/') ? join(ctx.root, dirname(rel)) : ctx.root;
  const pointers = pointersOf(body);
  let resolved = 0;
  for (const { token, at } of pointers) {
    const target = relative(ctx.root, resolve(at ? atBase : ctx.root, token));
    if (await exists(ctx, target)) resolved += 1;
    else problem(ctx, rel, `points at ${at ? '@' : ''}${token}, which does not exist`);
  }
  if (pointers.length === 0) {
    problem(ctx, rel, 'must point at a canonical file (AGENTS.md or .claude/rules/...)');
  } else if (resolved === 0) {
    problem(ctx, rel, 'none of its pointers resolve');
  }
}

function checkCursorMeta(ctx: Context, rel: string, name: string, data: Meta) {
  const always = data['alwaysApply'];
  if (!nonEmptyString(data['description'])) problem(ctx, rel, 'frontmatter needs a description');
  if (typeof always !== 'boolean') problem(ctx, rel, 'frontmatter needs alwaysApply: true/false');
  if (name === `${WORKFLOW}.mdc`) {
    if (always !== true) problem(ctx, rel, 'must set alwaysApply: true (always-on workflow rules)');
  } else if (
    always !== true &&
    patternsOf(data['globs']).length === 0 &&
    !AGENT_REQUESTED.includes(name.slice(0, -4))
  ) {
    problem(ctx, rel, 'needs globs (or alwaysApply: true); only agent-requested rules go without');
  }
}

type Meta = Record<string, unknown>;

function checkAntigravityMeta(ctx: Context, rel: string, name: string, data: Meta) {
  const trigger = data['trigger'];
  if (typeof trigger !== 'string' || !TRIGGERS.includes(trigger)) {
    const got = String(trigger);
    problem(ctx, rel, `trigger must be one of ${TRIGGERS.join(', ')} (got ${got}); rule dropped`);
  } else if (name === `${WORKFLOW}.md` && trigger !== 'always_on') {
    problem(ctx, rel, 'must set trigger: always_on (always-on workflow rules)');
  } else if (trigger === 'glob' && patternsOf(data['globs']).length === 0) {
    problem(ctx, rel, 'trigger: glob needs globs');
  } else if (trigger === 'model_decision' && !nonEmptyString(data['description'])) {
    problem(ctx, rel, 'trigger: model_decision needs a description');
  }
}

/** Cursor (`.mdc`) and Antigravity (`.md`) shims: valid frontmatter, short, pointing at real files. */
async function checkShims(ctx: Context): Promise<void> {
  for (const [dir, ext] of [
    ['.cursor/rules', '.mdc'],
    ['.agents/rules', '.md'],
  ] as const) {
    const cursor = dir === '.cursor/rules';
    for (const name of await list(ctx, dir)) {
      const rel = `${dir}/${name}`;
      if (!name.endsWith(ext)) {
        problem(ctx, rel, `${cursor ? 'Cursor' : 'Antigravity'} rules are ${ext} files`);
        continue;
      }
      const text = await read(ctx, rel);
      if (text === undefined) continue;
      checkLines(ctx, rel, text, SHIM_LINES);
      if (!cursor && text.length > ANTIGRAVITY_CHARS) {
        problem(ctx, rel, `${text.length} characters, budget is ${ANTIGRAVITY_CHARS}`);
      }
      const { data, body, error } = frontmatter(text);
      if (data === undefined) {
        problem(ctx, rel, error ?? 'needs frontmatter (Cursor: alwaysApply; Antigravity: trigger)');
        continue;
      }
      (cursor ? checkCursorMeta : checkAntigravityMeta)(ctx, rel, name, data);
      await checkPointers(ctx, rel, body);
    }
  }
}

/** Skills (`<dir>/<name>/SKILL.md`), commands and agents: the frontmatter each tool needs. */
async function checkCatalog(ctx: Context): Promise<void> {
  const entries: { rel: string; name: string | undefined }[] = [];
  for (const dir of ['.claude/skills', '.agents/skills']) {
    for (const name of await list(ctx, dir)) {
      if (!name.startsWith('.')) entries.push({ rel: `${dir}/${name}/SKILL.md`, name });
    }
  }
  for (const name of await list(ctx, '.claude/commands')) {
    if (name.endsWith('.md')) entries.push({ rel: `.claude/commands/${name}`, name: undefined });
  }
  for (const name of await list(ctx, '.claude/agents')) {
    const rel = `.claude/agents/${name}`;
    if (name.endsWith('.md')) entries.push({ rel, name: name.slice(0, -3) });
  }

  for (const { rel, name } of entries) {
    const text = await read(ctx, rel);
    if (text === undefined) {
      problem(ctx, rel, 'missing');
      continue;
    }
    checkLines(ctx, rel, text, SKILL_LINES);
    const { data, error } = frontmatter(text);
    if (data === undefined) {
      problem(ctx, rel, error ?? 'needs frontmatter with a description');
      continue;
    }
    if (name !== undefined && data['name'] !== name) {
      problem(ctx, rel, `frontmatter name must be "${name}" (the folder or file name)`);
    }
    if (!nonEmptyString(data['description'])) problem(ctx, rel, 'frontmatter needs a description');
  }
}

export async function checkRules(ctx: Context): Promise<void> {
  await checkClaudeRules(ctx);
  await checkShims(ctx);
  await checkCatalog(ctx);
}

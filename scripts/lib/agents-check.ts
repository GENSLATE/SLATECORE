/**
 * Agent-folder drift check (`AGENTS.md`, `CLAUDE.md`, `.claude/`, `.cursor/`, `.agents/`); `bun run
 * check` fails on any problem it returns. One rule text lives in `AGENTS.md` and `.claude/rules/`,
 * Cursor and Antigravity get short shims that point at it; this keeps the pieces from drifting:
 * a short core of required files, imports, shim targets, frontmatter a tool would silently drop,
 * size budgets, exec-form hooks, safe permissions and the owner's workflow rules. The folder scans
 * live in `agents-check/`. `checkAgents(root)` takes the root so tests break a scratch copy; every
 * problem starts with the path it is about.
 */
import type { Stats } from 'node:fs';
import { lstat } from 'node:fs/promises';
import { basename, join, resolve } from 'node:path';

import {
  type Context,
  checkLines,
  exists,
  frontmatter,
  list,
  patternsOf,
  problem,
  read,
} from './agents-check/context';
import { checkRules } from './agents-check/rules';
import { checkScreenshots } from './agents-check/screenshots';
import { checkMcp, checkSettings } from './agents-check/settings';

/** The first line of `AGENTS.md` (the naming statement every agent reads first). */
const AGENTS_STATEMENT =
  "SLATECORE is GENSLATE's suite of portable apps; this monorepo builds SLATECORE LAUNCHER by GENSLATE";

/** The only imports `CLAUDE.md` may contain: they load into every session. */
const CLAUDE_IMPORTS: readonly string[] = ['@AGENTS.md', '@.claude/memory/active-context.md'];

/** Files that must exist. Everything else is found by scanning the folders. */
const CORE: readonly string[] = [
  'AGENTS.md',
  'CLAUDE.md',
  '.mcp.json',
  '.cursorignore',
  '.vscode/extensions.json',
  '.claude/settings.json',
  '.claude/rules/branding.md',
  '.claude/rules/workflow.md',
  '.claude/memory/active-context.md',
  '.cursor/rules/workflow.mdc',
  '.agents/rules/workflow.md',
];

/** Files and folders no tool reads (research/agent-folders.md §2), with why they are rejected. */
const LEFTOVERS: Readonly<Record<string, string>> = {
  '.claudeignore':
    'Claude Code ignores it; use permissions.deny Read rules in .claude/settings.json',
  '.agentignore': 'no tool documents it',
  '.cursorrules': 'legacy; use .cursor/rules/*.mdc',
  '.claude/mcp.json': 'Claude Code reads the root .mcp.json',
  '.claude/personas': 'not a Claude Code folder',
  '.claude/workflows': 'Claude creates this folder itself',
  '.claude/memory/agent-memory.md': 'fold it into .claude/memory/active-context.md',
  '.cursor/settings.json': 'not a Cursor file; editor settings live in .vscode/',
  '.cursor/config.json': 'not a Cursor file',
  '.cursor/extensions.json': 'recommendations live in .vscode/extensions.json',
  '.agents/config.json': 'not an Antigravity file',
  '.agents/mcp.json': 'Antigravity reads .agents/mcp_config.json',
  '.agents/workflows': 'Antigravity retires workflows on 2026-11-01; use .agents/skills',
  '.agents/commands': 'Antigravity has no commands folder; use .agents/skills',
  '.agents/hooks': 'hooks are guard-only and live in .claude/hooks; lefthook and CI are the net',
  '.agents/personas': 'not an Antigravity folder',
  '.agents/memory': 'memory lives in .claude/memory',
};

/** Always-loaded files stay small (targets from research/agent-folders.md §5). */
const LINE_BUDGETS: Readonly<Record<string, number>> = {
  'AGENTS.md': 110,
  'CLAUDE.md': 30,
  '.claude/memory/active-context.md': 40,
};

/** The owner's standing workflow rules (plan first, screenshots) in AGENTS.md and workflow.md. */
const WORKFLOW_PHRASES = [
  'approval',
  '.claude/project/screenshots/',
  '<app>-<polar-night|snow-storm>-<view>.png',
  'Polar Night',
  'Snow Storm',
];

/** Text each file must keep: names in capitals, the owner's rules, the no-agent-credit rule. */
const MUST_MENTION: Readonly<Record<string, readonly string[]>> = {
  'AGENTS.md': [...WORKFLOW_PHRASES, 'Co-Authored-By'],
  '.claude/rules/workflow.md': WORKFLOW_PHRASES,
  '.claude/rules/branding.md': ['GENSLATE', 'SLATECORE', 'SLATECORE LAUNCHER'],
};

/** Local state, not repository content: never walked. */
const LOCAL_STATE = ['.claude/logs', '.claude/worktrees', '.cursor/hooks/state', 'node_modules'];

const AGENT_FILES = [
  'AGENTS.md',
  'CLAUDE.md',
  '.mcp.json',
  '.cursorignore',
  '.cursorindexingignore',
];
const AGENT_FOLDERS = ['.claude', '.cursor', '.agents', '.vscode'];

async function checkCore(ctx: Context): Promise<void> {
  for (const rel of CORE) if (!(await exists(ctx, rel))) problem(ctx, rel, 'missing');
  for (const [rel, why] of Object.entries(LEFTOVERS)) {
    if (await exists(ctx, rel)) problem(ctx, rel, `no tool reads this; delete it (${why})`);
  }
}

/** No symlinks (Windows clones turn them into text files) and no empty files. */
async function checkTree(ctx: Context): Promise<void> {
  const inspect = (rel: string, info: Stats): void => {
    if (info.isSymbolicLink()) {
      problem(ctx, rel, 'is a symlink; agent files are real files (Windows clones break symlinks)');
    } else if (info.isFile() && info.size === 0 && basename(rel) !== '.gitkeep') {
      if (!rel.startsWith('.claude/project/')) problem(ctx, rel, 'empty file; write or delete it');
    }
  };
  const walk = async (dir: string): Promise<void> => {
    for (const name of await list(ctx, dir)) {
      const rel = `${dir}/${name}`;
      if (LOCAL_STATE.some((skip) => rel === skip || rel.endsWith(`/${skip}`))) continue;
      const info = await lstat(join(ctx.root, rel));
      inspect(rel, info);
      if (info.isDirectory()) await walk(rel);
    }
  };
  for (const rel of [...AGENT_FILES, ...AGENT_FOLDERS]) {
    if (await exists(ctx, rel)) inspect(rel, await lstat(join(ctx.root, rel)));
  }
  for (const folder of AGENT_FOLDERS) await walk(folder);
}

/** `CLAUDE.md` imports `AGENTS.md` first and nothing but the active context besides. */
async function checkClaudeMd(ctx: Context): Promise<void> {
  const claude = await read(ctx, 'CLAUDE.md');
  if (claude === undefined) return; // Reported as missing.
  if (claude.split(/\r?\n/)[0]?.trim() !== '@AGENTS.md') {
    problem(ctx, 'CLAUDE.md', 'first line must be @AGENTS.md (Claude Code reads AGENTS.md via it)');
  }
  // An import is `@path` at the start of a word, anywhere in the text outside code spans and fences.
  const prose = claude.replace(/^(`{3,}|~{3,})[\s\S]*?^\1/gm, '').replace(/`[^`\n]*`/g, '');
  for (const match of prose.matchAll(/(?<![\w@])@([^\s)\]>"']+)/g)) {
    const path = (match[1] ?? '').replace(/[.,;:!?]+$/, '');
    if (!CLAUDE_IMPORTS.includes(`@${path}`)) {
      const allowed = CLAUDE_IMPORTS.join(' and ');
      problem(ctx, 'CLAUDE.md', `import @${path} is not allowed; only ${allowed} may load`);
    } else if (!(await exists(ctx, path))) {
      problem(ctx, 'CLAUDE.md', `import @${path} points at a missing file`);
    }
  }
}

/** The naming statement, the phrases each file must keep, and the branding rule's scope. */
async function checkTexts(ctx: Context): Promise<void> {
  for (const [rel, phrases] of Object.entries(MUST_MENTION)) {
    const text = await read(ctx, rel);
    if (text === undefined) continue; // Reported as missing.
    for (const phrase of phrases) {
      if (!text.includes(phrase)) problem(ctx, rel, `must mention "${phrase}"`);
    }
  }
  const agents = await read(ctx, 'AGENTS.md');
  if (agents !== undefined && !agents.split(/\r?\n/)[0]?.startsWith(AGENTS_STATEMENT)) {
    problem(ctx, 'AGENTS.md', `must open with "${AGENTS_STATEMENT}"`);
  }
  const branding = await read(ctx, '.claude/rules/branding.md');
  const glob = '**/*.{tsx,ts,rs,md,json,toml}';
  if (branding !== undefined && !patternsOf(frontmatter(branding).data?.['paths']).includes(glob)) {
    problem(ctx, '.claude/rules/branding.md', `paths must include ${glob} (user-visible strings)`);
  }
}

async function checkBudgets(ctx: Context): Promise<void> {
  for (const [rel, max] of Object.entries(LINE_BUDGETS)) {
    const text = await read(ctx, rel);
    if (text !== undefined) checkLines(ctx, rel, text, max);
  }
}

/** Returns every drift problem found under `root` (empty when the agent folders are consistent). */
export async function checkAgents(root: string): Promise<string[]> {
  const ctx: Context = { root: resolve(root), problems: [] };
  await checkCore(ctx);
  await checkTree(ctx);
  await checkClaudeMd(ctx);
  await checkTexts(ctx);
  await checkBudgets(ctx);
  await checkRules(ctx);
  await checkSettings(ctx);
  await checkMcp(ctx);
  await checkScreenshots(ctx);
  return ctx.problems;
}

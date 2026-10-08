/**
 * Agent-folder drift check (`AGENTS.md`, `CLAUDE.md`, `.claude/`, `.cursor/`, `.agents/`).
 * `bun run check` prints every returned problem and fails when there is one.
 *
 * One rule text lives in one place (`AGENTS.md` and `.claude/rules/`); Cursor and Antigravity get
 * short shims that point at it. This check keeps the pieces from drifting apart: imports, shim
 * targets, frontmatter that each tool silently drops when it is wrong, size budgets, hooks in exec
 * form, the PowerShell pin, and the owner's standing workflow rules (plan first, screenshots).
 *
 * `checkAgents(root)` takes the repository root so tests can break a scratch copy one thing at a
 * time. Every problem starts with the path it is about.
 */
// cspell:ignore claudeignore agentignore cursorignore cursorrules worktrees backticked
import { lstat, readdir, readFile } from 'node:fs/promises';
import { basename, dirname, join, relative, resolve } from 'node:path';

/** The first line of `AGENTS.md` (the naming statement every agent reads first). */
export const AGENTS_STATEMENT =
  "SLATECORE is GENSLATE's suite of portable apps; this monorepo builds SLATECORE LAUNCHER by GENSLATE";

/** The only imports `CLAUDE.md` may contain. */
const CLAUDE_IMPORTS: readonly string[] = ['@AGENTS.md', '@.claude/memory/active-context.md'];

/** Rules without `paths:` frontmatter: always loaded (the owner's standing workflow rules). */
const ALWAYS_ON_RULES: readonly string[] = ['workflow'];

/** Cursor rules without `globs` that the agent requests by description. */
const AGENT_REQUESTED_CURSOR_RULES: readonly string[] = ['changes'];

/** Antigravity `trigger` values; a rule without one of them is silently dropped. */
const ANTIGRAVITY_TRIGGERS: readonly string[] = ['always_on', 'model_decision', 'glob', 'manual'];

/** Antigravity limit per rule file, in characters. */
const ANTIGRAVITY_RULE_CHARS = 12_000;

/** Lines a Cursor or Antigravity shim may take: pointers, not prose. */
const SHIM_LINES = 15;

const CLAUDE_RULES = [
  'branding',
  'design-contract',
  'motion-polish',
  'typescript-standards',
  'rust-standards',
  'portability-no-trace',
  'vault-security',
  'monorepo-structure',
  'testing',
  'changes',
  'workflow',
] as const;

const CLAUDE_SKILLS = ['new-component', 'design-tokens', 'new-app', 'release'] as const;
const CLAUDE_COMMANDS = ['check', 'change'] as const;
const CLAUDE_AGENTS = [
  'code-reviewer',
  'security-reviewer',
  'ui-visual-qa',
  'design-system-engineer',
  'tauri-rust-engineer',
] as const;
const CLAUDE_HOOKS = [
  'hook.shared',
  'format-on-edit.hook',
  'guard-generated.hook',
  'session-start.hook',
] as const;
const CLAUDE_MEMORY = ['active-context', 'decisions', 'lessons-learned', 'specifications'] as const;
const CURSOR_RULES = [
  'workflow',
  'branding',
  'design-system',
  'typescript',
  'rust',
  'portability',
  'vault-security',
  'changes',
] as const;
const ANTIGRAVITY_RULES = [
  'workflow',
  'branding',
  'design-system',
  'typescript',
  'rust-portability',
  'changes',
] as const;
const SHARED_SKILLS = ['check', 'run-tests', 'ui-fixes', 'change'] as const;

/** Files that must exist. A deleted rule or shim is drift. */
const REQUIRED: readonly string[] = [
  '.mcp.json',
  '.cursorignore',
  '.vscode/extensions.json',
  '.claude/settings.json',
  '.claude/README.md',
  ...CLAUDE_RULES.map((name) => `.claude/rules/${name}.md`),
  ...CLAUDE_SKILLS.map((name) => `.claude/skills/${name}/SKILL.md`),
  ...CLAUDE_COMMANDS.map((name) => `.claude/commands/${name}.md`),
  ...CLAUDE_AGENTS.map((name) => `.claude/agents/${name}.md`),
  ...CLAUDE_HOOKS.map((name) => `.claude/hooks/${name}.ts`),
  ...CLAUDE_MEMORY.map((name) => `.claude/memory/${name}.md`),
  '.cursor/README.md',
  '.cursor/mcp.json',
  ...CURSOR_RULES.map((name) => `.cursor/rules/${name}.mdc`),
  '.agents/README.md',
  '.agents/mcp_config.json',
  ...ANTIGRAVITY_RULES.map((name) => `.agents/rules/${name}.md`),
  ...SHARED_SKILLS.map((name) => `.agents/skills/${name}/SKILL.md`),
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
  '.cursor/extesions.json': 'typo and not a Cursor file',
  '.agents/config.json': 'not an Antigravity file',
  '.agents/mcp.json': 'Antigravity reads .agents/mcp_config.json',
  '.agents/workflows': 'Antigravity retires workflows on 2026-11-01; use .agents/skills',
  '.agents/commands': 'Antigravity has no commands folder; use .agents/skills',
  '.agents/hooks': 'hooks are guard-only and live in .claude/hooks; lefthook and CI are the net',
  '.agents/personas': 'not an Antigravity folder',
  '.agents/memory': 'memory lives in .claude/memory',
};

/** Line budgets by path (targets from research/agent-folders.md §5, not tool limits). */
const LINE_BUDGETS: Readonly<Record<string, number>> = {
  'AGENTS.md': 110,
  'CLAUDE.md': 30,
  '.claude/README.md': 40,
  '.cursor/README.md': 25,
  '.agents/README.md': 25,
  '.claude/memory/active-context.md': 40,
  '.claude/memory/decisions.md': 250,
  '.claude/memory/lessons-learned.md': 120,
  '.claude/memory/specifications.md': 150,
  '.claude/commands/check.md': 25,
  '.claude/commands/change.md': 20,
  '.claude/skills/new-component/SKILL.md': 150,
  '.claude/skills/design-tokens/SKILL.md': 90,
  '.claude/skills/new-app/SKILL.md': 120,
  '.claude/skills/release/SKILL.md': 100,
  '.claude/agents/code-reviewer.md': 50,
  '.claude/agents/security-reviewer.md': 60,
  '.claude/agents/ui-visual-qa.md': 60,
  '.claude/agents/design-system-engineer.md': 35,
  '.claude/agents/tauri-rust-engineer.md': 40,
  '.agents/skills/check/SKILL.md': 40,
  '.agents/skills/run-tests/SKILL.md': 70,
  '.agents/skills/ui-fixes/SKILL.md': 80,
  '.agents/skills/change/SKILL.md': 30,
};

/** Skills and agents without an entry above: Claude's own guidance is 500 lines. */
const DEFAULT_LINES = 500;

/** Character budgets of the Claude rules (they load into context, so they stay small). */
const RULE_CHARS: Readonly<Record<string, number>> = {
  'design-contract': 11_500,
  'motion-polish': 5_000,
  'typescript-standards': 3_000,
  'rust-standards': 3_500,
  'portability-no-trace': 4_500,
  'vault-security': 4_500,
  'monorepo-structure': 3_000,
  testing: 2_500,
  changes: 1_500,
  branding: 3_000,
  workflow: 3_000,
};

/** Text the owner's standing workflow rules must keep (AGENTS.md and workflow.md both). */
const WORKFLOW_PHRASES: readonly string[] = [
  'approval',
  '.claude/project/screenshots/',
  '<app>-<polar-night|snow-storm>-<view>.png',
  'Polar Night',
  'Snow Storm',
];

/** Package managers the bun-only rule forbids. */
const DENIED_TOOLS: readonly string[] = ['npm', 'npx', 'pnpm', 'yarn', 'node'];

/** Folders whose contents are local state, not repository content. */
const LOCAL_STATE = ['.claude/logs', '.claude/worktrees', '.cursor/hooks/state', 'node_modules'];

const SCREENSHOT_ROOT = '.claude/project/screenshots';
const THEMES = ['polar-night', 'snow-storm'] as const;
const KEBAB = '[a-z0-9]+(?:-[a-z0-9]+)*';

interface Context {
  readonly root: string;
  readonly problems: string[];
}

interface Frontmatter {
  readonly data: Record<string, unknown> | undefined;
  readonly body: string;
  readonly error: string | undefined;
}

const escapeRegExp = (text: string): string => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const nonEmptyString = (value: unknown): value is string =>
  typeof value === 'string' && value.trim() !== '';

function problem(ctx: Context, file: string, message: string): void {
  ctx.problems.push(`${file}: ${message}`);
}

async function exists(ctx: Context, rel: string): Promise<boolean> {
  try {
    await lstat(join(ctx.root, rel));
    return true;
  } catch {
    return false;
  }
}

async function read(ctx: Context, rel: string): Promise<string | undefined> {
  try {
    return await readFile(join(ctx.root, rel), 'utf8');
  } catch {
    return undefined;
  }
}

async function list(ctx: Context, rel: string): Promise<string[]> {
  try {
    return (await readdir(join(ctx.root, rel))).sort();
  } catch {
    return [];
  }
}

async function readJson(ctx: Context, rel: string): Promise<Record<string, unknown> | undefined> {
  const text = await read(ctx, rel);
  if (text === undefined) return undefined; // `required files` reports a missing file.
  try {
    const parsed: unknown = JSON.parse(text);
    if (isObject(parsed)) return parsed;
    problem(ctx, rel, 'must be a JSON object');
  } catch (error) {
    problem(
      ctx,
      rel,
      `is not valid JSON (${error instanceof Error ? error.message : String(error)})`,
    );
  }
  return undefined;
}

/** Splits `---` frontmatter from the body and parses it as YAML. */
function frontmatter(text: string): Frontmatter {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/.exec(text);
  if (match === null) return { data: undefined, body: text, error: undefined };
  const body = text.slice(match[0].length);
  try {
    const parsed: unknown = Bun.YAML.parse(match[1] ?? '');
    if (isObject(parsed)) return { data: parsed, body, error: undefined };
    return { data: undefined, body, error: 'frontmatter must be a YAML mapping' };
  } catch (error) {
    return {
      data: undefined,
      body,
      error: `frontmatter is not valid YAML (${error instanceof Error ? error.message : String(error)})`,
    };
  }
}

const lineCount = (text: string): number => text.replace(/\r?\n$/, '').split(/\r?\n/).length;

function patternsOf(value: unknown): string[] {
  if (typeof value === 'string')
    return value
      .split(',')
      .map((part) => part.trim())
      .filter(Boolean);
  if (Array.isArray(value)) return value.filter(nonEmptyString);
  return [];
}

/** Every entry below `rel` (relative, `/` separators), without following links. */
async function walk(
  ctx: Context,
  rel: string,
): Promise<{ readonly rel: string; readonly symlink: boolean; readonly emptyFile: boolean }[]> {
  const found: { rel: string; symlink: boolean; emptyFile: boolean }[] = [];
  const visit = async (dir: string): Promise<void> => {
    let names: string[];
    try {
      names = await readdir(join(ctx.root, dir));
    } catch {
      return;
    }
    for (const name of names.sort()) {
      const path = `${dir}/${name}`;
      if (LOCAL_STATE.some((skip) => path === skip || path.endsWith(`/${skip}`))) continue;
      const info = await lstat(join(ctx.root, path));
      found.push({
        rel: path,
        symlink: info.isSymbolicLink(),
        emptyFile: info.isFile() && info.size === 0,
      });
      if (info.isDirectory()) await visit(path);
    }
  };
  await visit(rel);
  return found;
}

async function checkRequired(ctx: Context): Promise<void> {
  for (const rel of REQUIRED) {
    if (!(await exists(ctx, rel))) problem(ctx, rel, 'missing');
  }
}

async function checkLeftovers(ctx: Context): Promise<void> {
  for (const [rel, why] of Object.entries(LEFTOVERS)) {
    if (await exists(ctx, rel)) problem(ctx, rel, `no tool reads this; delete it (${why})`);
  }
}

/** No symlinks (Windows clones turn them into text files) and no empty files. */
async function checkTree(ctx: Context): Promise<void> {
  const roots = ['AGENTS.md', 'CLAUDE.md', '.mcp.json', '.cursorignore', '.vscode'];
  for (const rel of roots) {
    try {
      if ((await lstat(join(ctx.root, rel))).isSymbolicLink()) {
        problem(
          ctx,
          rel,
          'is a symlink; agent files are real files (Windows clones break symlinks)',
        );
      }
    } catch {
      // Missing files are reported by `checkRequired`.
    }
  }
  for (const folder of ['.claude', '.cursor', '.agents']) {
    for (const entry of await walk(ctx, folder)) {
      if (entry.symlink) {
        problem(
          ctx,
          entry.rel,
          'is a symlink; agent files are real files (Windows clones break symlinks)',
        );
      } else if (
        entry.emptyFile &&
        basename(entry.rel) !== '.gitkeep' &&
        !entry.rel.startsWith('.claude/project/')
      ) {
        problem(
          ctx,
          entry.rel,
          'empty file; a tool would silently ignore it, write it or delete it',
        );
      }
    }
  }
}

async function checkRootFiles(ctx: Context): Promise<void> {
  const claude = await read(ctx, 'CLAUDE.md');
  if (claude !== undefined) {
    if (claude.split(/\r?\n/)[0]?.trim() !== '@AGENTS.md') {
      problem(
        ctx,
        'CLAUDE.md',
        'first line must be @AGENTS.md (Claude Code reads AGENTS.md only through the import)',
      );
    }
    for (const line of claude.split(/\r?\n/)) {
      if (!line.startsWith('@')) continue;
      const target = line.trim();
      if (!CLAUDE_IMPORTS.includes(target)) {
        problem(
          ctx,
          'CLAUDE.md',
          `import ${target} is not allowed; only ${CLAUDE_IMPORTS.join(' and ')} load every session`,
        );
      } else if (!(await exists(ctx, target.slice(1)))) {
        problem(ctx, 'CLAUDE.md', `import ${target} points at a missing file`);
      }
    }
  } else {
    problem(ctx, 'CLAUDE.md', 'missing');
  }

  const agents = await read(ctx, 'AGENTS.md');
  if (agents === undefined) {
    problem(ctx, 'AGENTS.md', 'missing');
  } else if (!(agents.split(/\r?\n/)[0] ?? '').startsWith(AGENTS_STATEMENT)) {
    problem(ctx, 'AGENTS.md', `must open with "${AGENTS_STATEMENT}"`);
  }
}

async function checkBranding(ctx: Context): Promise<void> {
  const rel = '.claude/rules/branding.md';
  const text = await read(ctx, rel);
  if (text === undefined) return; // Reported as missing.
  for (const name of ['GENSLATE', 'SLATECORE', 'SLATECORE LAUNCHER']) {
    if (!text.includes(name)) problem(ctx, rel, `must name ${name} (in capitals)`);
  }
  const { data } = frontmatter(text);
  if (!patternsOf(data?.['paths']).includes('**/*.{tsx,ts,rs,md,json,toml}')) {
    problem(
      ctx,
      rel,
      'paths must include **/*.{tsx,ts,rs,md,json,toml} so user-visible strings are checked',
    );
  }
}

async function checkWorkflow(ctx: Context): Promise<void> {
  for (const rel of ['AGENTS.md', '.claude/rules/workflow.md']) {
    const text = await read(ctx, rel);
    if (text === undefined) continue;
    for (const phrase of WORKFLOW_PHRASES) {
      if (!text.includes(phrase)) {
        problem(
          ctx,
          rel,
          `the owner's workflow rules (plan first, screenshots) must mention "${phrase}"`,
        );
      }
    }
  }

  const cursor = await read(ctx, '.cursor/rules/workflow.mdc');
  if (cursor !== undefined && frontmatter(cursor).data?.['alwaysApply'] !== true) {
    problem(
      ctx,
      '.cursor/rules/workflow.mdc',
      'must set alwaysApply: true (the workflow rules are always on)',
    );
  }
  const antigravity = await read(ctx, '.agents/rules/workflow.md');
  if (antigravity !== undefined && frontmatter(antigravity).data?.['trigger'] !== 'always_on') {
    problem(
      ctx,
      '.agents/rules/workflow.md',
      'must set trigger: always_on (the workflow rules are always on)',
    );
  }
}

async function checkClaudeRules(ctx: Context): Promise<void> {
  for (const name of await list(ctx, '.claude/rules')) {
    if (name.startsWith('.')) continue;
    const rel = `.claude/rules/${name}`;
    if (name.endsWith('.mdc') || !name.endsWith('.md')) {
      problem(ctx, rel, 'Claude Code loads only .md files from .claude/rules; rename it to .md');
      continue;
    }
    const text = await read(ctx, rel);
    if (text === undefined) continue;
    const { data, error } = frontmatter(text);
    if (error !== undefined) problem(ctx, rel, error);
    if (ALWAYS_ON_RULES.includes(name.slice(0, -3))) continue;
    if (patternsOf(data?.['paths']).length === 0) {
      problem(
        ctx,
        rel,
        'needs paths: frontmatter (a list of globs), or it loads into every session',
      );
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
    if (
      /^(?:\.claude|\.cursor|\.agents)\/[^*\s<>]+\.[a-z]+$/.test(token) ||
      /^(?:AGENTS|CLAUDE)\.md$/.test(token)
    ) {
      pointers.push({ token, at: false });
    }
  }
  return pointers;
}

/** Every shim names at least one canonical file and every file it names exists. */
async function checkShimPointers(ctx: Context, rel: string, body: string): Promise<void> {
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

async function checkCursorRules(ctx: Context): Promise<void> {
  for (const name of await list(ctx, '.cursor/rules')) {
    const rel = `.cursor/rules/${name}`;
    if (!name.endsWith('.mdc')) {
      problem(ctx, rel, 'Cursor rules are .mdc files');
      continue;
    }
    const text = await read(ctx, rel);
    if (text === undefined) continue;
    const { data, body, error } = frontmatter(text);
    if (data === undefined) {
      problem(ctx, rel, error ?? 'needs frontmatter (description, globs, alwaysApply)');
      continue;
    }
    if (!nonEmptyString(data['description'])) problem(ctx, rel, 'frontmatter needs a description');
    if (typeof data['alwaysApply'] !== 'boolean')
      problem(ctx, rel, 'frontmatter needs alwaysApply: true or false');
    const always = data['alwaysApply'] === true;
    const globs = patternsOf(data['globs']);
    if (
      !always &&
      globs.length === 0 &&
      !AGENT_REQUESTED_CURSOR_RULES.includes(name.slice(0, -4))
    ) {
      problem(
        ctx,
        rel,
        'needs globs (or alwaysApply: true); only agent-requested rules go without',
      );
    }
    await checkShimPointers(ctx, rel, body);
  }
}

async function checkAntigravityRules(ctx: Context): Promise<void> {
  for (const name of await list(ctx, '.agents/rules')) {
    const rel = `.agents/rules/${name}`;
    if (!name.endsWith('.md')) {
      problem(ctx, rel, 'Antigravity rules are .md files');
      continue;
    }
    const text = await read(ctx, rel);
    if (text === undefined) continue;
    if (text.length > ANTIGRAVITY_RULE_CHARS) {
      problem(
        ctx,
        rel,
        `${text.length} characters, budget is ${ANTIGRAVITY_RULE_CHARS} (the tool's limit)`,
      );
    }
    const { data, body, error } = frontmatter(text);
    if (data === undefined) {
      problem(
        ctx,
        rel,
        error ?? 'needs frontmatter with a trigger (Antigravity drops the rule otherwise)',
      );
      continue;
    }
    const trigger = data['trigger'];
    if (typeof trigger !== 'string' || !ANTIGRAVITY_TRIGGERS.includes(trigger)) {
      problem(
        ctx,
        rel,
        `trigger must be one of ${ANTIGRAVITY_TRIGGERS.join(', ')} (got ${String(trigger)}); the rule is dropped otherwise`,
      );
    } else if (trigger === 'glob' && patternsOf(data['globs']).length === 0) {
      problem(ctx, rel, 'trigger: glob needs globs');
    } else if (trigger === 'model_decision' && !nonEmptyString(data['description'])) {
      problem(ctx, rel, 'trigger: model_decision needs a description');
    }
    await checkShimPointers(ctx, rel, body);
  }
}

/** Skills (`<dir>/<name>/SKILL.md`), commands and agents: the frontmatter each tool needs. */
async function checkSkillsCommandsAgents(ctx: Context): Promise<void> {
  for (const dir of ['.claude/skills', '.agents/skills']) {
    for (const name of await list(ctx, dir)) {
      if (name.startsWith('.')) continue;
      const rel = `${dir}/${name}/SKILL.md`;
      const text = await read(ctx, rel);
      if (text === undefined) {
        problem(ctx, `${dir}/${name}`, 'missing SKILL.md');
        continue;
      }
      const { data, error } = frontmatter(text);
      if (data === undefined) {
        problem(ctx, rel, error ?? 'needs frontmatter with name and description');
        continue;
      }
      if (data['name'] !== name)
        problem(ctx, rel, `frontmatter name must be "${name}" (the folder name)`);
      if (!nonEmptyString(data['description']))
        problem(ctx, rel, 'frontmatter needs a description');
    }
  }

  for (const [dir, needsName] of [
    ['.claude/commands', false],
    ['.claude/agents', true],
  ] as const) {
    for (const name of await list(ctx, dir)) {
      if (!name.endsWith('.md')) continue;
      const rel = `${dir}/${name}`;
      const text = await read(ctx, rel);
      if (text === undefined) continue;
      const { data, error } = frontmatter(text);
      if (data === undefined) {
        problem(ctx, rel, error ?? 'needs frontmatter');
        continue;
      }
      if (needsName && data['name'] !== name.slice(0, -3)) {
        problem(ctx, rel, `frontmatter name must be "${name.slice(0, -3)}" (the file name)`);
      }
      if (!nonEmptyString(data['description']))
        problem(ctx, rel, 'frontmatter needs a description');
    }
  }
}

async function checkBudgets(ctx: Context): Promise<void> {
  const lineBudget = (rel: string): number | undefined => {
    const direct = LINE_BUDGETS[rel];
    if (direct !== undefined) return direct;
    if (/^\.(?:claude|agents)\/skills\/[^/]+\/SKILL\.md$/.test(rel)) return DEFAULT_LINES;
    if (/^\.claude\/agents\/[^/]+\.md$/.test(rel)) return DEFAULT_LINES;
    if (/^\.cursor\/rules\/[^/]+\.mdc$/.test(rel) || /^\.agents\/rules\/[^/]+\.md$/.test(rel)) {
      return SHIM_LINES;
    }
    return undefined;
  };

  const files: string[] = [...Object.keys(LINE_BUDGETS)];
  for (const [dir, suffix] of [
    ['.cursor/rules', '.mdc'],
    ['.agents/rules', '.md'],
    ['.claude/agents', '.md'],
  ] as const) {
    for (const name of await list(ctx, dir))
      if (name.endsWith(suffix)) files.push(`${dir}/${name}`);
  }
  for (const dir of ['.claude/skills', '.agents/skills']) {
    for (const name of await list(ctx, dir)) files.push(`${dir}/${name}/SKILL.md`);
  }

  for (const rel of new Set(files)) {
    const text = await read(ctx, rel);
    const budget = lineBudget(rel);
    if (text === undefined || budget === undefined) continue;
    const lines = lineCount(text);
    if (lines > budget) problem(ctx, rel, `${lines} lines, budget is ${budget}`);
  }

  for (const name of await list(ctx, '.claude/rules')) {
    if (!name.endsWith('.md')) continue;
    const text = await read(ctx, `.claude/rules/${name}`);
    const budget = RULE_CHARS[name.slice(0, -3)] ?? 6_000;
    if (text !== undefined && text.length > budget) {
      problem(ctx, `.claude/rules/${name}`, `${text.length} characters, budget is ${budget}`);
    }
  }
}

/** A hook handler runs `bun <script>` without a shell, and the script exists. */
async function checkHandler(ctx: Context, where: string, handler: unknown): Promise<void> {
  if (!isObject(handler)) {
    problem(ctx, '.claude/settings.json', `${where} must be an object`);
    return;
  }
  const command = handler['command'];
  const args = handler['args'];
  if (command !== 'bun' || !Array.isArray(args) || typeof args[0] !== 'string') {
    problem(
      ctx,
      '.claude/settings.json',
      `${where} must use exec form: "command": "bun" plus "args": ["\${CLAUDE_PROJECT_DIR}/<script>"] (got ${JSON.stringify(command)})`,
    );
    return;
  }
  const prefix = `\${CLAUDE_PROJECT_DIR}/`;
  const script = args[0];
  if (!script.startsWith(prefix)) {
    problem(ctx, '.claude/settings.json', `${where} script must start with ${prefix}`);
  } else if (!(await exists(ctx, script.slice(prefix.length)))) {
    problem(
      ctx,
      '.claude/settings.json',
      `${where} runs ${script.slice(prefix.length)}, which does not exist`,
    );
  }
}

async function checkSettings(ctx: Context): Promise<void> {
  const rel = '.claude/settings.json';
  const settings = await readJson(ctx, rel);
  if (settings === undefined) return;

  const attribution = settings['attribution'];
  if (
    !isObject(attribution) ||
    attribution['commit'] !== '' ||
    attribution['pr'] !== '' ||
    attribution['sessionUrl'] !== false
  ) {
    problem(
      ctx,
      rel,
      'attribution must be {"commit": "", "pr": "", "sessionUrl": false} (no agent credit)',
    );
  }

  const env = settings['env'];
  if (!isObject(env) || env['CLAUDE_CODE_USE_POWERSHELL_TOOL'] !== '0') {
    problem(
      ctx,
      rel,
      'env.CLAUDE_CODE_USE_POWERSHELL_TOOL must be "0" so every Bash(...) rule applies',
    );
  }

  const permissions = settings['permissions'];
  const deny =
    isObject(permissions) && Array.isArray(permissions['deny']) ? permissions['deny'] : [];
  for (const tool of DENIED_TOOLS) {
    for (const shell of ['Bash', 'PowerShell']) {
      const rule = `${shell}(${tool} *)`;
      if (!deny.includes(rule))
        problem(ctx, rel, `permissions.deny must contain "${rule}" (bun only)`);
    }
  }

  const hooks = settings['hooks'];
  if (isObject(hooks)) {
    for (const [event, entries] of Object.entries(hooks)) {
      for (const [index, entry] of (Array.isArray(entries) ? entries : []).entries()) {
        const handlers = isObject(entry) && Array.isArray(entry['hooks']) ? entry['hooks'] : [];
        for (const [position, handler] of handlers.entries()) {
          await checkHandler(ctx, `hooks.${event}[${index}].hooks[${position}]`, handler);
        }
      }
    }
  }
  if (settings['statusLine'] !== undefined)
    await checkHandler(ctx, 'statusLine', settings['statusLine']);
}

/** MCP files: valid JSON, and a key is always read from the environment, never written down. */
async function checkMcp(ctx: Context): Promise<void> {
  for (const rel of ['.mcp.json', '.cursor/mcp.json', '.agents/mcp_config.json']) {
    const config = await readJson(ctx, rel);
    if (config === undefined) continue;
    const servers = config['mcpServers'];
    if (!isObject(servers)) {
      problem(ctx, rel, 'needs an "mcpServers" object');
      continue;
    }
    for (const [name, server] of Object.entries(servers)) {
      if (!isObject(server)) continue;
      for (const field of ['headers', 'env']) {
        const values = server[field];
        if (!isObject(values)) continue;
        for (const [key, value] of Object.entries(values)) {
          if (typeof value !== 'string' || !/\$\{[^}]+\}/.test(value)) {
            problem(
              ctx,
              rel,
              `server "${name}" ${field}.${key} must read an environment variable (\${NAME}), never a literal secret`,
            );
          }
        }
      }
    }
  }
}

/** `.claude/project/screenshots/<app>/<app>-<polar-night|snow-storm>-<view>.png`, both themes. */
async function checkScreenshots(ctx: Context): Promise<void> {
  if (!(await exists(ctx, SCREENSHOT_ROOT))) return;
  for (const app of await list(ctx, SCREENSHOT_ROOT)) {
    const appRel = `${SCREENSHOT_ROOT}/${app}`;
    const info = await lstat(join(ctx.root, appRel));
    if (!info.isDirectory()) {
      if (app !== '.gitkeep') {
        problem(ctx, appRel, `must live in ${SCREENSHOT_ROOT}/<app>/, one folder per app`);
      }
      continue;
    }
    if (!new RegExp(`^${KEBAB}$`).test(app)) {
      problem(ctx, appRel, 'the app folder name must be lowercase kebab-case');
    }
    const name = new RegExp(`^${escapeRegExp(app)}-(${THEMES.join('|')})-(${KEBAB})\\.png$`);
    const seen = new Set<string>();
    for (const file of await list(ctx, appRel)) {
      if (file === '.gitkeep') continue;
      const match = name.exec(file);
      if (match === null) {
        const image = /\.(?:png|jpe?g|webp|gif)$/i.test(file);
        if (image) {
          problem(
            ctx,
            `${appRel}/${file}`,
            `name must be ${app}-<polar-night|snow-storm>-<view>.png (lowercase kebab-case view)`,
          );
        }
        continue;
      }
      seen.add(`${match[1]}/${match[2]}`);
    }
    for (const key of seen) {
      const [theme = '', view = ''] = key.split('/');
      for (const other of THEMES.filter((candidate) => candidate !== theme)) {
        if (!seen.has(`${other}/${view}`)) {
          problem(
            ctx,
            `${appRel}/${app}-${other}-${view}.png`,
            'missing; every view is captured in both themes (Polar Night and Snow Storm)',
          );
        }
      }
    }
  }
}

/** Returns every drift problem found under `root` (empty when the agent folders are consistent). */
export async function checkAgents(root: string): Promise<string[]> {
  const ctx: Context = { root: resolve(root), problems: [] };
  await checkRequired(ctx);
  await checkLeftovers(ctx);
  await checkTree(ctx);
  await checkRootFiles(ctx);
  await checkBranding(ctx);
  await checkWorkflow(ctx);
  await checkClaudeRules(ctx);
  await checkCursorRules(ctx);
  await checkAntigravityRules(ctx);
  await checkSkillsCommandsAgents(ctx);
  await checkBudgets(ctx);
  await checkSettings(ctx);
  await checkMcp(ctx);
  await checkScreenshots(ctx);
  return ctx.problems;
}

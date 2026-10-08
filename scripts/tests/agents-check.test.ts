// cspell:ignore claudeignore agentignore cursorignore cursorrules NOSYSTEM
import { afterAll, describe, expect, test } from 'bun:test';
import { cp, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { devNull, tmpdir } from 'node:os';
import { dirname, join, relative } from 'node:path';

import { checkAgents } from '../lib/agents-check';
import { ROOT } from '../lib/paths';

/** Everything the check reads, copied into a scratch root so tests can break one thing at a time. */
const COPIED = [
  'AGENTS.md',
  'CLAUDE.md',
  '.mcp.json',
  '.cursorignore',
  '.vscode',
  '.claude',
  '.cursor',
  '.agents',
] as const;

const scratch: string[] = [];

afterAll(async () => {
  await Promise.all(scratch.map((dir) => rm(dir, { recursive: true, force: true })));
});

/** A copy of the agent files without the (large) screenshot folder. */
async function fixture(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), 'slatecore-agents-'));
  scratch.push(dir);
  for (const name of COPIED) {
    await cp(join(ROOT, name), join(dir, name), {
      recursive: true,
      filter: (source) =>
        !relative(ROOT, source).replaceAll('\\', '/').startsWith('.claude/project/screenshots'),
    });
  }
  return dir;
}

async function edit(root: string, path: string, change: (text: string) => string): Promise<void> {
  const file = join(root, path);
  await writeFile(file, change(await readFile(file, 'utf8')));
}

async function put(root: string, path: string, text: string): Promise<void> {
  await mkdir(dirname(join(root, path)), { recursive: true });
  await writeFile(join(root, path), text);
}

/** Problems whose text mentions every needle (case-insensitive). */
function about(problems: readonly string[], ...needles: string[]): string[] {
  return problems.filter((problem) =>
    needles.every((needle) => problem.toLowerCase().includes(needle.toLowerCase())),
  );
}

async function readText(path: string): Promise<string> {
  return readFile(join(ROOT, path), 'utf8');
}

const lines = (text: string): string[] => text.replace(/\n$/, '').split('\n');

describe('the repository agent folders', () => {
  test('repo_agent_folders_have_no_problems', async () => {
    expect(await checkAgents(ROOT)).toEqual([]);
  });

  test('a_pristine_copy_has_no_problems', async () => {
    expect(await checkAgents(await fixture())).toEqual([]);
  });
});

describe('AGENTS.md and CLAUDE.md', () => {
  test('claude_md_imports_agents_md', async () => {
    expect(lines(await readText('CLAUDE.md'))[0]).toBe('@AGENTS.md');

    const root = await fixture();
    await edit(root, 'CLAUDE.md', (text) => text.replace('@AGENTS.md', 'See AGENTS.md'));
    expect(about(await checkAgents(root), 'CLAUDE.md', '@AGENTS.md')).not.toEqual([]);
  });

  test('claude_md_imports_the_active_context_and_nothing_else', async () => {
    const text = await readText('CLAUDE.md');
    const imports = lines(text).filter((line) => line.startsWith('@'));
    expect(imports).toEqual(['@AGENTS.md', '@.claude/memory/active-context.md']);

    const root = await fixture();
    await edit(root, 'CLAUDE.md', (body) => `${body}@.claude/memory/decisions.md\n`);
    expect(about(await checkAgents(root), 'CLAUDE.md', 'import')).not.toEqual([]);
  });

  test('agents_md_opens_with_slatecore_by_genslate_statement', async () => {
    const first = lines(await readText('AGENTS.md'))[0] ?? '';
    expect(first).toStartWith(
      "SLATECORE is GENSLATE's suite of portable apps; this monorepo builds SLATECORE LAUNCHER by GENSLATE",
    );

    const root = await fixture();
    await edit(root, 'AGENTS.md', (text) => `# Rules\n\n${text}`);
    expect(about(await checkAgents(root), 'AGENTS.md', 'SLATECORE')).not.toEqual([]);
  });

  test('size_budgets_respected', async () => {
    expect(lines(await readText('AGENTS.md')).length).toBeLessThanOrEqual(110);
    expect(lines(await readText('CLAUDE.md')).length).toBeLessThanOrEqual(30);
    expect(lines(await readText('.claude/memory/active-context.md')).length).toBeLessThanOrEqual(
      40,
    );

    const root = await fixture();
    // 110 lines are fine, 111 are not.
    const head = lines(await readFile(join(root, 'AGENTS.md'), 'utf8')).slice(0, 1);
    await writeFile(
      join(root, 'AGENTS.md'),
      `${[...head, ...Array.from({ length: 109 }, (_, i) => `- rule ${i}`)].join('\n')}\n`,
    );
    expect(about(await checkAgents(root), 'AGENTS.md', 'lines')).toEqual([]);
    await edit(root, 'AGENTS.md', (text) => `${text}- one more\n`);
    expect(about(await checkAgents(root), 'AGENTS.md', '111')).not.toEqual([]);

    const fat = await fixture();
    await edit(fat, '.cursor/rules/rust.mdc', (text) => `${text}${'a\n'.repeat(20)}`);
    expect(about(await checkAgents(fat), 'rust.mdc', 'lines')).not.toEqual([]);

    const big = await fixture();
    await edit(big, '.agents/rules/changes.md', (text) => `${text}${'x'.repeat(12_000)}\n`);
    expect(about(await checkAgents(big), '.agents/rules/changes.md', '12000')).not.toEqual([]);
  });
});

describe('branding', () => {
  test('branding_rule_exists_and_names_genslate_slatecore_launcher', async () => {
    const rule = await readText('.claude/rules/branding.md');
    for (const name of ['GENSLATE', 'SLATECORE', 'SLATECORE LAUNCHER'])
      expect(rule).toContain(name);
    expect(rule).toContain('slatecore-launcher.exe');
    expect(rule).toContain('xyz.genslate.slatecore.launcher');
    expect(rule).toContain('**/*.{tsx,ts,rs,md,json,toml}');

    const missing = await fixture();
    await rm(join(missing, '.claude/rules/branding.md'));
    expect(about(await checkAgents(missing), 'branding.md')).not.toEqual([]);

    const wrong = await fixture();
    await edit(wrong, '.claude/rules/branding.md', (text) =>
      text.replaceAll('SLATECORE LAUNCHER', 'Slatecore Launcher'),
    );
    expect(about(await checkAgents(wrong), 'branding.md', 'SLATECORE LAUNCHER')).not.toEqual([]);
  });
});

describe('workflow rules (owner standing rules)', () => {
  test('plan_first_and_screenshots_are_in_agents_md_and_the_workflow_rule', async () => {
    for (const path of ['AGENTS.md', '.claude/rules/workflow.md']) {
      const text = await readText(path);
      expect(text).toContain('approval');
      expect(text).toContain('.claude/project/screenshots/');
      expect(text).toContain('<app>-<polar-night|snow-storm>-<view>.png');
      expect(text).toContain('Polar Night');
      expect(text).toContain('Snow Storm');
    }

    const root = await fixture();
    await edit(root, '.claude/rules/workflow.md', (text) =>
      text.replaceAll('.claude/project/screenshots/', 'screenshots/'),
    );
    expect(about(await checkAgents(root), 'workflow.md', 'screenshots')).not.toEqual([]);

    const noPlan = await fixture();
    await edit(noPlan, 'AGENTS.md', (text) => text.replaceAll('approval', 'ok'));
    expect(about(await checkAgents(noPlan), 'AGENTS.md', 'approval')).not.toEqual([]);
  });

  test('workflow_shims_are_always_on', async () => {
    expect(await readText('.cursor/rules/workflow.mdc')).toContain('alwaysApply: true');
    expect(await readText('.agents/rules/workflow.md')).toContain('trigger: always_on');

    const cursor = await fixture();
    await edit(cursor, '.cursor/rules/workflow.mdc', (text) =>
      text.replace('alwaysApply: true', 'alwaysApply: false'),
    );
    expect(about(await checkAgents(cursor), 'workflow.mdc', 'alwaysApply')).not.toEqual([]);

    const antigravity = await fixture();
    await edit(antigravity, '.agents/rules/workflow.md', (text) =>
      text.replace('trigger: always_on', 'trigger: manual'),
    );
    expect(about(await checkAgents(antigravity), 'workflow.md', 'always_on')).not.toEqual([]);
  });
});

describe('structure', () => {
  test('no_symlinks_in_agent_folders', async () => {
    const root = await fixture();
    try {
      await symlink(join(root, 'AGENTS.md'), join(root, '.claude/rules/linked.md'));
    } catch {
      // Windows without Developer Mode cannot create symlinks; nothing to assert then.
      return;
    }
    expect(about(await checkAgents(root), 'linked.md', 'symlink')).not.toEqual([]);
  });

  test('rule_files_have_md_extension_in_claude_rules', async () => {
    const root = await fixture();
    await put(root, '.claude/rules/extra.mdc', '---\npaths:\n  - "**/*.ts"\n---\nbody\n');
    expect(about(await checkAgents(root), 'extra.mdc', '.md')).not.toEqual([]);

    const unscoped = await fixture();
    await put(unscoped, '.claude/rules/extra.md', 'always loaded, no paths\n');
    expect(about(await checkAgents(unscoped), 'extra.md', 'paths')).not.toEqual([]);

    const glob = await fixture();
    await edit(glob, '.claude/rules/rust-standards.md', (text) =>
      text.replace(/paths:[\s\S]*?---/, 'paths: []\n---'),
    );
    expect(about(await checkAgents(glob), 'rust-standards.md', 'paths')).not.toEqual([]);
  });

  test('antigravity_rules_have_valid_trigger', async () => {
    const root = await fixture();
    await edit(root, '.agents/rules/design-system.md', (text) =>
      text.replace('trigger: glob', 'trigger: alwaysOn'),
    );
    expect(about(await checkAgents(root), 'design-system.md', 'trigger')).not.toEqual([]);

    const bare = await fixture();
    await put(bare, '.agents/rules/bare.md', 'No frontmatter at all.\n');
    expect(about(await checkAgents(bare), 'bare.md', 'frontmatter')).not.toEqual([]);

    const noGlobs = await fixture();
    await edit(noGlobs, '.agents/rules/design-system.md', (text) =>
      text.replace(/^globs:.*\n/m, ''),
    );
    expect(about(await checkAgents(noGlobs), 'design-system.md', 'globs')).not.toEqual([]);

    const noDescription = await fixture();
    await edit(noDescription, '.agents/rules/changes.md', (text) =>
      text.replace(/^description:.*\n/m, ''),
    );
    expect(about(await checkAgents(noDescription), 'changes.md', 'description')).not.toEqual([]);

    const workflows = await fixture();
    await put(workflows, '.agents/workflows/old.md', '# deprecated\n');
    expect(about(await checkAgents(workflows), '.agents/workflows')).not.toEqual([]);
  });

  test('cursor_rules_have_valid_frontmatter', async () => {
    const root = await fixture();
    await edit(root, '.cursor/rules/typescript.mdc', (text) =>
      text.replace(/^globs:[\s\S]*?(?=alwaysApply)/m, ''),
    );
    expect(about(await checkAgents(root), 'typescript.mdc', 'globs')).not.toEqual([]);

    const plain = await fixture();
    await put(plain, '.cursor/rules/plain.md', '---\ndescription: x\n---\nbody\n');
    expect(about(await checkAgents(plain), 'plain.md', '.mdc')).not.toEqual([]);
  });

  test('shims_point_at_existing_files', async () => {
    const cursor = await fixture();
    await edit(cursor, '.cursor/rules/design-system.mdc', (text) =>
      text.replaceAll(
        '.claude/rules/design-contract.md',
        '.claude/rules/design-contract-missing.md',
      ),
    );
    expect(
      about(await checkAgents(cursor), 'design-system.mdc', 'design-contract-missing.md'),
    ).not.toEqual([]);

    const antigravity = await fixture();
    await edit(antigravity, '.agents/rules/design-system.md', (text) =>
      text.replaceAll('../../.claude/rules/motion-polish.md', '../.claude/rules/motion-polish.md'),
    );
    expect(
      about(await checkAgents(antigravity), 'design-system.md', 'motion-polish.md'),
    ).not.toEqual([]);

    const orphan = await fixture();
    await put(
      orphan,
      '.cursor/rules/orphan.mdc',
      '---\ndescription: x\nalwaysApply: false\n---\nNo pointer.\n',
    );
    expect(about(await checkAgents(orphan), 'orphan.mdc', 'point')).not.toEqual([]);
  });

  test('skills_commands_and_agents_have_the_frontmatter_their_tool_needs', async () => {
    const skill = await fixture();
    await edit(skill, '.claude/skills/release/SKILL.md', (text) =>
      text.replace(/^name:.*\n/m, 'name: other\n'),
    );
    expect(about(await checkAgents(skill), 'release/SKILL.md', 'name')).not.toEqual([]);

    const shared = await fixture();
    await edit(shared, '.agents/skills/check/SKILL.md', (text) =>
      text.replace(/^description:.*\n/m, ''),
    );
    expect(about(await checkAgents(shared), 'check/SKILL.md', 'description')).not.toEqual([]);

    const agent = await fixture();
    await edit(agent, '.claude/agents/code-reviewer.md', (text) => text.replace(/^name:.*\n/m, ''));
    expect(about(await checkAgents(agent), 'code-reviewer.md', 'name')).not.toEqual([]);

    const long = await fixture();
    await edit(long, '.claude/skills/release/SKILL.md', (text) => `${text}${'step\n'.repeat(600)}`);
    expect(about(await checkAgents(long), 'release/SKILL.md', 'lines')).not.toEqual([]);
  });

  test('files_nobody_reads_and_empty_files_are_rejected', async () => {
    for (const path of ['.claudeignore', '.cursorrules', '.agentignore', '.claude/mcp.json']) {
      const root = await fixture();
      await put(root, path, '{}\n');
      expect(about(await checkAgents(root), path)).not.toEqual([]);
    }

    const empty = await fixture();
    await put(empty, '.claude/rules/empty.md', '');
    expect(about(await checkAgents(empty), 'empty.md', 'empty')).not.toEqual([]);

    const keep = await fixture();
    await put(keep, '.claude/logs/.gitkeep', '');
    expect(about(await checkAgents(keep), '.gitkeep')).toEqual([]);

    const mcp = await fixture();
    await edit(mcp, '.mcp.json', (text) =>
      text.replace(/\$\{CONTEXT7_API_KEY\}/, 'ctx7sk-live-abc123'),
    );
    expect(about(await checkAgents(mcp), '.mcp.json', 'environment variable')).not.toEqual([]);
  });

  test('required_files_exist', async () => {
    const root = await fixture();
    await rm(join(root, '.claude/agents/security-reviewer.md'));
    await rm(join(root, '.agents/skills/ui-fixes', 'SKILL.md'));
    const problems = await checkAgents(root);
    expect(about(problems, 'security-reviewer.md', 'missing')).not.toEqual([]);
    expect(about(problems, 'ui-fixes', 'missing')).not.toEqual([]);
  });
});

describe('settings and hooks', () => {
  test('hook_scripts_exist_and_use_exec_form', async () => {
    const settings = JSON.parse(await readText('.claude/settings.json')) as {
      hooks: Record<string, { hooks: { type: string; command: string; args: string[] }[] }[]>;
    };
    const handlers = Object.values(settings.hooks).flatMap((entries) =>
      entries.flatMap((entry) => entry.hooks),
    );
    expect(handlers.length).toBeGreaterThanOrEqual(3);
    for (const handler of handlers) {
      expect(handler.command).toBe('bun');
      const script = (handler.args[0] ?? '').replace(`\${CLAUDE_PROJECT_DIR}`, ROOT);
      expect(await Bun.file(script).exists()).toBe(true);
    }

    const shell = await fixture();
    await edit(shell, '.claude/settings.json', (text) =>
      text.replace('"command": "bun"', '"command": "bun .claude/hooks/format-on-edit.hook.ts"'),
    );
    expect(about(await checkAgents(shell), 'settings.json', 'exec form')).not.toEqual([]);

    const gone = await fixture();
    await rm(join(gone, '.claude/hooks/guard-generated.hook.ts'));
    expect(about(await checkAgents(gone), 'guard-generated.hook.ts')).not.toEqual([]);
  });

  test('settings_pin_powershell_tool_off_and_deny_npm_variants', async () => {
    const settings = JSON.parse(await readText('.claude/settings.json')) as {
      attribution: { commit: string; pr: string; sessionUrl: boolean };
      env: Record<string, string>;
      permissions: { deny: string[] };
    };
    expect(settings.env['CLAUDE_CODE_USE_POWERSHELL_TOOL']).toBe('0');
    expect(settings.attribution).toEqual({ commit: '', pr: '', sessionUrl: false });
    for (const tool of ['npm', 'npx', 'pnpm', 'yarn', 'node']) {
      expect(settings.permissions.deny).toContain(`Bash(${tool} *)`);
      expect(settings.permissions.deny).toContain(`PowerShell(${tool} *)`);
    }

    const noEnv = await fixture();
    await edit(noEnv, '.claude/settings.json', (text) =>
      text.replace(
        '"CLAUDE_CODE_USE_POWERSHELL_TOOL": "0"',
        '"CLAUDE_CODE_USE_POWERSHELL_TOOL": "1"',
      ),
    );
    expect(about(await checkAgents(noEnv), 'settings.json', 'POWERSHELL')).not.toEqual([]);

    const noDeny = await fixture();
    await edit(noDeny, '.claude/settings.json', (text) => text.replace('"PowerShell(npx *)",', ''));
    expect(about(await checkAgents(noDeny), 'settings.json', 'PowerShell(npx *)')).not.toEqual([]);

    const noBash = await fixture();
    await edit(noBash, '.claude/settings.json', (text) => text.replace('"Bash(yarn *)",', ''));
    expect(about(await checkAgents(noBash), 'settings.json', 'Bash(yarn *)')).not.toEqual([]);

    const credit = await fixture();
    await edit(credit, '.claude/settings.json', (text) =>
      text.replace('"sessionUrl": false', '"sessionUrl": true'),
    );
    expect(about(await checkAgents(credit), 'settings.json', 'attribution')).not.toEqual([]);
  });
});

describe('.claude/project (screenshots reused for READMEs and websites)', () => {
  const PNG = Buffer.from('89504e470d0a1a0a', 'hex');

  async function shot(root: string, app: string, name: string): Promise<void> {
    await put(root, `.claude/project/screenshots/${app}/${name}`, PNG.toString('latin1'));
  }

  test('project_screenshots_folder_is_accepted', async () => {
    const root = await fixture();
    await put(root, '.claude/project/.gitkeep', '');
    await shot(root, 'launcher', 'launcher-polar-night-settings-about.png');
    await shot(root, 'launcher', 'launcher-snow-storm-settings-about.png');
    await shot(root, 'design-kit', 'design-kit-polar-night-actions-button.png');
    await shot(root, 'design-kit', 'design-kit-snow-storm-actions-button.png');
    expect(await checkAgents(root)).toEqual([]);
  });

  test('screenshots_follow_the_naming_convention', async () => {
    const wrongName = await fixture();
    await shot(wrongName, 'launcher', 'about-dark.png');
    expect(about(await checkAgents(wrongName), 'about-dark.png', 'launcher-<')).not.toEqual([]);

    const wrongFolder = await fixture();
    await shot(wrongFolder, 'notes', 'launcher-polar-night-home.png');
    expect(about(await checkAgents(wrongFolder), 'launcher-polar-night-home.png')).not.toEqual([]);

    const stray = await fixture();
    await put(stray, '.claude/project/screenshots/loose.png', 'x');
    expect(about(await checkAgents(stray), 'loose.png')).not.toEqual([]);
  });

  test('every_screenshot_exists_in_both_themes', async () => {
    const root = await fixture();
    await shot(root, 'launcher', 'launcher-polar-night-pinned.png');
    const problems = await checkAgents(root);
    expect(about(problems, 'launcher-snow-storm-pinned.png', 'missing')).not.toEqual([]);

    await shot(root, 'launcher', 'launcher-snow-storm-pinned.png');
    expect(await checkAgents(root)).toEqual([]);
  });

  test('the_committed_screenshots_pass_when_present', async () => {
    // The real folder holds the Design Kit and launcher sets; the repo check above already walks it.
    expect(await Bun.file(join(ROOT, '.claude/project/.gitkeep')).exists()).toBe(true);
  });
});

describe('hook scripts', () => {
  const HOOKS = join(ROOT, '.claude', 'hooks');

  function runHook(
    script: string,
    stdin: string,
    env: Record<string, string> = {},
    cwd: string = ROOT,
  ): { code: number; stdout: string; stderr: string } {
    const result = Bun.spawnSync(['bun', join(HOOKS, script)], {
      cwd,
      env: { ...process.env, CLAUDE_PROJECT_DIR: cwd, ...env },
      stdin: new TextEncoder().encode(stdin),
      stdout: 'pipe',
      stderr: 'pipe',
    });
    return {
      code: result.exitCode,
      stdout: result.stdout.toString(),
      stderr: result.stderr.toString(),
    };
  }

  const edit = (file: string, tool = 'Edit'): string =>
    JSON.stringify({
      hook_event_name: 'PreToolUse',
      tool_name: tool,
      tool_input: { file_path: file },
    });

  test('guard_generated_blocks_edits_to_generated_files_with_exit_2', () => {
    for (const path of [
      'bun.lock',
      'Cargo.lock',
      'packages/tokens/src/generated/css/tokens.css',
      'crates/design-tokens/src/generated/theme.rs',
      'packages/design-system/src/icons/codicon-names.generated.ts',
      'programs/desktop/launcher/src-tauri/gen/schemas/desktop-schema.json',
      'programs/desktop/launcher/installDir/storage/vault/files/0123.gvf',
      'release/slatecore-launcher-0.1.0-base.zip',
    ]) {
      const result = runHook('guard-generated.hook.ts', edit(join(ROOT, path)));
      expect(result.code).toBe(2);
      expect(result.stderr.length).toBeGreaterThan(20);
    }
  });

  test('guard_generated_understands_windows_paths_and_relative_paths', () => {
    expect(
      runHook('guard-generated.hook.ts', edit('S:\\SLATECORE\\bun.lock'), {
        CLAUDE_PROJECT_DIR: 'S:\\SLATECORE',
      }).code,
    ).toBe(2);
    expect(
      runHook('guard-generated.hook.ts', edit('packages\\tokens\\src\\generated\\css\\tokens.css'))
        .code,
    ).toBe(2);
    expect(runHook('guard-generated.hook.ts', edit('Cargo.lock', 'Write')).code).toBe(2);
  });

  test('guard_generated_lets_source_files_through', () => {
    for (const path of [
      'packages/tokens/src/tokens/color.tokens.ts',
      'packages/design-system/src/components/actions/button/button.component.tsx',
      'AGENTS.md',
      'Cargo.toml',
      'package.json',
    ]) {
      const result = runHook('guard-generated.hook.ts', edit(join(ROOT, path)));
      expect(result.code).toBe(0);
    }
  });

  test('guard_generated_fails_open_on_bad_input', () => {
    expect(runHook('guard-generated.hook.ts', '').code).toBe(0);
    expect(runHook('guard-generated.hook.ts', 'not json').code).toBe(0);
    expect(runHook('guard-generated.hook.ts', '{"tool_input":{}}').code).toBe(0);
  });

  test('format_on_edit_ignores_files_it_cannot_format_and_fails_open', () => {
    const md = runHook('format-on-edit.hook.ts', edit(join(ROOT, 'AGENTS.md'), 'Edit'));
    expect(md.code).toBe(0);
    expect(md.stdout).toBe('');
    expect(runHook('format-on-edit.hook.ts', 'nope').code).toBe(0);
    const missing = runHook('format-on-edit.hook.ts', edit(join(ROOT, 'does/not/exist.ts')));
    expect(missing.code).toBe(0);
  });

  test('session_start_prints_context_and_replaces_an_agent_git_identity', async () => {
    const repo = await mkdtemp(join(tmpdir(), 'slatecore-session-'));
    scratch.push(repo);
    const git = (...args: string[]): string => {
      const result = Bun.spawnSync(['git', ...args], {
        cwd: repo,
        env: { ...process.env, GIT_CONFIG_GLOBAL: devNull, GIT_CONFIG_NOSYSTEM: '1' },
        stdout: 'pipe',
        stderr: 'pipe',
      });
      return result.stdout.toString().trim();
    };
    git('init', '-q', '-b', 'main');
    git(
      '-c',
      'user.name=Jo Writer',
      '-c',
      'user.email=jo@example.com',
      'commit',
      '-q',
      '--allow-empty',
      '-m',
      'chore(repo): start',
    );
    git('config', 'user.name', 'Claude');
    git('config', 'user.email', 'noreply@anthropic.com');

    const result = runHook(
      'session-start.hook.ts',
      '{"hook_event_name":"SessionStart","source":"startup"}',
      { GIT_CONFIG_GLOBAL: devNull, GIT_CONFIG_NOSYSTEM: '1' },
      repo,
    );
    expect(result.code).toBe(0);
    const output = JSON.parse(result.stdout) as {
      hookSpecificOutput: { hookEventName: string; additionalContext: string };
    };
    expect(output.hookSpecificOutput.hookEventName).toBe('SessionStart');
    expect(output.hookSpecificOutput.additionalContext).toContain('main');
    expect(git('config', 'user.name')).toBe('Jo Writer');
    expect(git('config', 'user.email')).toBe('jo@example.com');
  });

  test('session_start_leaves_a_human_identity_alone', async () => {
    const repo = await mkdtemp(join(tmpdir(), 'slatecore-session-'));
    scratch.push(repo);
    const env = { GIT_CONFIG_GLOBAL: devNull, GIT_CONFIG_NOSYSTEM: '1' };
    const git = (...args: string[]): string =>
      Bun.spawnSync(['git', ...args], {
        cwd: repo,
        env: { ...process.env, ...env },
        stdout: 'pipe',
      })
        .stdout.toString()
        .trim();
    git('init', '-q', '-b', 'main');
    git('config', 'user.name', 'Sam Owner');
    git('config', 'user.email', 'sam@example.com');
    const result = runHook('session-start.hook.ts', '{}', env, repo);
    expect(result.code).toBe(0);
    expect(git('config', 'user.name')).toBe('Sam Owner');
  });
});

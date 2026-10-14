import { afterEach, describe, expect, test } from 'bun:test';
import { mkdir, mkdtemp, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  addWorkspaceDependency,
  addWorkspaceMember,
  nextPort,
  scaffoldApp,
  titleCase,
} from '../lib/new-app';
import { ROOT } from '../lib/paths';
import { renderString } from '../lib/template';

const TEMPLATES = join(ROOT, 'scripts/templates');

const temporary: string[] = [];

/** A temp repo copy with the real root Cargo.toml and the launcher's dev port. */
async function repoCopy(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), 'slatecore-new-app-'));
  temporary.push(root);
  await writeFile(join(root, 'Cargo.toml'), await Bun.file(join(ROOT, 'Cargo.toml')).text());
  const launcher = join(root, 'programs/desktop/launcher');
  await mkdir(join(launcher, 'src-tauri'), { recursive: true });
  await writeFile(
    join(launcher, 'src-tauri/tauri.conf.json'),
    JSON.stringify({ build: { devUrl: 'http://localhost:1420' } }),
  );
  await writeFile(join(launcher, 'package.json'), JSON.stringify({ name: '@genslate/launcher' }));
  return root;
}

async function text(root: string, file: string): Promise<string> {
  return await Bun.file(join(root, file)).text();
}

async function allFiles(dir: string): Promise<string[]> {
  const entries = await readdir(dir, { withFileTypes: true, recursive: true });
  return entries
    .filter((entry) => entry.isFile())
    .map((entry) => join(entry.parentPath, entry.name));
}

afterEach(async () => {
  for (const dir of temporary.splice(0)) await rm(dir, { recursive: true, force: true });
});

describe('new-app', () => {
  test('new_app_scaffolds_into_programs_desktop_and_crates_core', async () => {
    const root = await repoCopy();
    const result = await scaffoldApp(root, TEMPLATES, { name: 'notes' });

    for (const file of [
      'programs/desktop/notes/package.json',
      'programs/desktop/notes/index.html',
      'programs/desktop/notes/vite.config.ts',
      'programs/desktop/notes/src/main.tsx',
      'programs/desktop/notes/src/app/app.meta.ts',
      'programs/desktop/notes/src/assets/app-icon.svg',
      'programs/desktop/notes/src-tauri/Cargo.toml',
      'programs/desktop/notes/src-tauri/tauri.conf.json',
      'programs/desktop/notes/src-tauri/capabilities/main.capability.json',
      'programs/desktop/notes/src-tauri/src/lib.rs',
      'programs/desktop/notes/tests/unit/app.test.tsx',
      'crates/notes-core/Cargo.toml',
      'crates/notes-core/src/lib.rs',
    ]) {
      expect(await Bun.file(join(root, file)).exists()).toBe(true);
    }
    expect(result.written.length).toBeGreaterThan(20);

    // The Cargo workspace knows both new crates, in sorted order, and still parses.
    const cargo = await text(root, 'Cargo.toml');
    const parsed = Bun.TOML.parse(cargo) as {
      workspace: { members: string[]; dependencies: Record<string, { path: string }> };
    };
    expect(parsed.workspace.members).toContain('crates/notes-core');
    expect(parsed.workspace.members).toContain('programs/desktop/notes/src-tauri');
    expect([...parsed.workspace.members].sort()).toEqual(parsed.workspace.members);
    expect(parsed.workspace.dependencies['genslate-notes-core']?.path).toBe('crates/notes-core');

    // Crate and package names.
    expect(await text(root, 'crates/notes-core/Cargo.toml')).toContain(
      'name = "genslate-notes-core"',
    );
    const shell = await text(root, 'programs/desktop/notes/src-tauri/Cargo.toml');
    expect(shell).toContain('name = "genslate-notes"');
    expect(shell).toContain('name = "slatecore-notes"');
    expect(shell).toContain('genslate-notes-core.workspace = true');
    expect(await text(root, 'programs/desktop/notes/src-tauri/src/main.rs')).toContain(
      'genslate_notes_lib::run()',
    );
    expect(await text(root, 'programs/desktop/notes/src-tauri/src/commands/app_info.rs')).toContain(
      'use genslate_notes_core::AppInfo;',
    );
    const pkg = JSON.parse(await text(root, 'programs/desktop/notes/package.json')) as {
      name: string;
      scripts: Record<string, string>;
    };
    expect(pkg.name).toBe('@genslate/notes');
    expect(Object.keys(pkg.scripts)).toEqual(
      expect.arrayContaining(['dev', 'dev:web', 'build', 'typecheck', 'test']),
    );
  });

  test('new_app_scaffold_is_titled_slatecore_name_and_credits_genslate', async () => {
    const root = await repoCopy();
    await scaffoldApp(root, TEMPLATES, { name: 'code-review' });
    const app = 'programs/desktop/code-review';

    const conf = JSON.parse(await text(root, `${app}/src-tauri/tauri.conf.json`)) as {
      productName: string;
      identifier: string;
      mainBinaryName: string;
      app: { windows: { title: string }[] };
      bundle: { publisher: string };
    };
    expect(conf.productName).toBe('SLATECORE Code Review');
    expect(conf.app.windows[0]?.title).toBe('SLATECORE Code Review');
    expect(conf.identifier).toBe('xyz.genslate.slatecore.codereview');
    expect(conf.mainBinaryName).toBe('slatecore-code-review');
    expect(conf.bundle.publisher).toBe('GENSLATE');

    expect(await text(root, `${app}/index.html`)).toContain('<title>SLATECORE Code Review</title>');
    const meta = await text(root, `${app}/src/app/app.meta.ts`);
    expect(meta).toContain("productName: 'SLATECORE Code Review'");
    expect(meta).toContain("developer: 'GENSLATE'");
    expect(await text(root, `${app}/src/features/home/home.component.tsx`)).toContain(
      'by {APP.developer}',
    );
    expect(await text(root, `${app}/tests/unit/app.test.tsx`)).toContain(
      "getByText('by GENSLATE')",
    );
    expect(await text(root, `${app}/README.md`)).toContain('A SLATECORE app by GENSLATE');
    expect(await text(root, `${app}/src/assets/app-icon.svg`)).toContain(
      '<title>SLATECORE Code Review</title>',
    );
  });

  test('new_app_renders_every_tag_and_carries_no_personal_or_old_names', async () => {
    const root = await repoCopy();
    await scaffoldApp(root, TEMPLATES, { name: 'notes' });
    const files = [
      ...(await allFiles(join(root, 'programs/desktop/notes'))),
      ...(await allFiles(join(root, 'crates/notes-core'))),
    ];
    for (const file of files) {
      const content = await Bun.file(file).text();
      expect(content).not.toContain('{{');
      expect(content).not.toMatch(/GENSLATE Notes|moon/i);
      expect(file.endsWith('.tera')).toBe(false);
    }
  });

  test('new_app_picks_free_dev_ports_and_refuses_collisions', async () => {
    const root = await repoCopy();
    await scaffoldApp(root, TEMPLATES, { name: 'notes' });
    expect(await text(root, 'programs/desktop/notes/src-tauri/tauri.conf.json')).toContain(
      'http://localhost:1440',
    );
    expect(await text(root, 'programs/desktop/notes/vite.config.ts')).toContain('port: 1440');

    await scaffoldApp(root, TEMPLATES, { name: 'second' });
    expect(await text(root, 'programs/desktop/second/vite.config.ts')).toContain('port: 1450');

    await expect(scaffoldApp(root, TEMPLATES, { name: 'third', port: 1441 })).rejects.toThrow(
      'port',
    );
    await expect(scaffoldApp(root, TEMPLATES, { name: 'notes' })).rejects.toThrow('not empty');
    await expect(scaffoldApp(root, TEMPLATES, { name: 'launcher' })).rejects.toThrow();
    await expect(scaffoldApp(root, TEMPLATES, { name: 'Bad_Name' })).rejects.toThrow('kebab-case');
  });

  test('cargo_helpers_are_idempotent_and_keep_order', () => {
    const cargo = [
      '[workspace]',
      'members = [',
      '    "crates/a",',
      '    "programs/x",',
      ']',
      '',
      '[workspace.dependencies]',
      'genslate-a = { path = "crates/a" }',
      'genslate-z = { path = "crates/z" }',
      '',
      'serde = "1"',
      '',
    ].join('\n');
    const withMember = addWorkspaceMember(cargo, 'crates/b');
    expect(withMember).toContain('    "crates/a",\n    "crates/b",\n    "programs/x",');
    expect(addWorkspaceMember(withMember, 'crates/b')).toBe(withMember);
    const withDep = addWorkspaceDependency(cargo, 'genslate-m-core', 'crates/m-core');
    expect(withDep.split('\n').slice(-6, -3)).toEqual([
      'genslate-a = { path = "crates/a" }',
      'genslate-m-core = { path = "crates/m-core" }',
      'genslate-z = { path = "crates/z" }',
    ]);
    expect(addWorkspaceDependency(withDep, 'genslate-m-core', 'crates/m-core')).toBe(withDep);
  });

  test('naming_and_port_helpers', () => {
    expect(titleCase('code-review')).toBe('Code Review');
    expect(nextPort([1420, 1430])).toBe(1440);
    expect(nextPort([1420, 1430, 1440, 1450])).toBe(1460);
  });

  test('template_renderer_supports_arithmetic_and_filters', () => {
    const vars = { name: 'code-review', port: 1440 };
    expect(renderString('{{ name | replace(from="-", to="_") }} {{ port + 1 }}', vars)).toBe(
      'code_review 1441',
    );
    expect(() => renderString('{{ missing }}', vars)).toThrow('unknown variable');
  });
});

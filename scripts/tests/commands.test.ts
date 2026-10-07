import { afterEach, describe, expect, test } from 'bun:test';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { planClean, removePaths } from '../lib/clean';
import { ROOT } from '../lib/paths';
import { bunProblem, readBunPin, readRustPin, rustProblem } from '../lib/toolchain';
import { turboCommand } from '../lib/turbo';
import {
  nextVersion,
  planVersionBump,
  setCargoWorkspaceVersion,
  writeVersionEdits,
} from '../lib/version';

const temporary: string[] = [];

async function tempDir(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), 'slatecore-scripts-'));
  temporary.push(dir);
  return dir;
}

async function write(root: string, file: string, text = ''): Promise<void> {
  const path = join(root, file);
  await mkdir(join(path, '..'), { recursive: true });
  await writeFile(path, text);
}

async function exists(root: string, file: string): Promise<boolean> {
  return await Bun.file(join(root, file)).exists();
}

afterEach(async () => {
  for (const dir of temporary.splice(0)) await rm(dir, { recursive: true, force: true });
});

describe('turbo helper', () => {
  test('run_turbo_passes_extra_args', () => {
    const { argv, env } = turboCommand(
      ['run', 'check', '--force', '--filter=@genslate/tokens'],
      'bun',
    );
    expect(argv).toEqual([
      'bun',
      'x',
      '--no-install',
      'turbo',
      'run',
      'check',
      '--force',
      '--filter=@genslate/tokens',
    ]);
    expect(env).toEqual({ TURBO_TELEMETRY_DISABLED: '1' });
  });

  test('run_turbo_runs_the_pinned_turbo_and_returns_its_exit_code', async () => {
    const script = `import { runTurbo } from './scripts/commands/turbo.util.ts';
      process.exit(await runTurbo(['--version']));`;
    const child = Bun.spawn([process.execPath, '-e', script], {
      cwd: ROOT,
      stdout: 'pipe',
      stderr: 'pipe',
    });
    const [out, code] = await Promise.all([new Response(child.stdout).text(), child.exited]);
    const pin = (await Bun.file(join(ROOT, 'package.json')).json()) as {
      workspaces: { catalog: { turbo: string } };
    };
    expect(code).toBe(0);
    expect(out.trim()).toBe(pin.workspaces.catalog.turbo);
  });
});

describe('setup toolchain checks', () => {
  test('setup_checks_bun_version_matches_package_manager_pin', async () => {
    const packageJson = await Bun.file(join(ROOT, 'package.json')).text();
    const pin = readBunPin(packageJson);
    expect(pin).toMatch(/^\d+\.\d+\.\d+$/);
    // The running bun is the pinned one (devEngines refuses to install otherwise).
    expect(bunProblem(pin, Bun.version)).toBeUndefined();
    expect(bunProblem(pin, '1.0.0')).toContain(`bun@${pin}`);
    expect(() => readBunPin('{"packageManager":"bun@^1.4"}')).toThrow('exact');
    expect(() => readBunPin('{}')).toThrow('packageManager');
  });

  test('setup_checks_rust_version_matches_rust_toolchain_pin', async () => {
    const pin = readRustPin(await Bun.file(join(ROOT, 'rust-toolchain.toml')).text());
    expect(rustProblem(pin, `rustc ${pin} (abc1234 2026-01-01)`)).toBeUndefined();
    expect(rustProblem(pin, 'rustc 1.0.0 (abc1234 2020-01-01)')).toContain(pin);
    expect(() => readRustPin('[toolchain]\nchannel = "stable"\n')).toThrow('exact');
  });
});

describe('clean', () => {
  async function fixture(): Promise<string> {
    const root = await tempDir();
    await write(root, 'release/.gitkeep');
    await write(root, 'release/.archive/.gitkeep');
    await write(root, 'release/.archive/slatecore-launcher-0.0.9-base.zip', 'old');
    await write(root, 'release/slatecore-launcher-0.1.0-base.zip', 'new');
    await write(root, 'target/.gitkeep');
    await write(root, 'target/debug/build.log', 'x');
    await write(root, '.turbo/cache/a.tar.zst', 'x');
    await write(root, 'coverage/lcov.info', 'x');
    await write(root, 'tsconfig.tsbuildinfo', '{}');
    await write(root, 'node_modules/left-pad/index.js', 'x');
    await write(root, 'programs/webapp/example/dist/index.html', 'x');
    await write(root, 'programs/webapp/example/package.json', '{}');
    await write(root, 'programs/webapp/example/node_modules/x/index.js', 'x');
    await write(root, 'programs/desktop/launcher/src-tauri/gen/schemas/a.json', '{}');
    await write(root, 'programs/desktop/launcher/src-tauri/tauri.conf.json', '{}');
    await write(root, 'packages/tokens/.turbo/turbo-test.log', 'x');
    await write(root, 'packages/tokens/src/index.ts', 'export {};');
    return root;
  }

  test('clean_keeps_release_archive', async () => {
    const root = await fixture();
    const failed = await removePaths(await planClean(root, { deep: false, keepTarget: false }));

    expect(failed).toEqual([]);
    expect(await exists(root, 'release/.archive/slatecore-launcher-0.0.9-base.zip')).toBe(true);
    expect(await exists(root, 'release/.archive/.gitkeep')).toBe(true);
    expect(await exists(root, 'release/.gitkeep')).toBe(true);
    expect(await exists(root, 'release/slatecore-launcher-0.1.0-base.zip')).toBe(false);
  });

  test('clean_keeps_the_tracked_target_placeholder', async () => {
    const root = await fixture();
    await removePaths(await planClean(root, { deep: false, keepTarget: false }));

    expect(await exists(root, 'target/.gitkeep')).toBe(true);
    expect(await exists(root, 'target/debug/build.log')).toBe(false);
  });

  test('clean_removes_build_output_but_keeps_sources_and_dependencies', async () => {
    const root = await fixture();
    await removePaths(await planClean(root, { deep: false, keepTarget: false }));

    for (const gone of [
      '.turbo',
      'coverage',
      'tsconfig.tsbuildinfo',
      'programs/webapp/example/dist',
      'programs/desktop/launcher/src-tauri/gen',
      'packages/tokens/.turbo',
    ]) {
      expect(await exists(root, gone)).toBe(false);
    }
    for (const kept of [
      'node_modules/left-pad/index.js',
      'programs/webapp/example/node_modules/x/index.js',
      'programs/webapp/example/package.json',
      'programs/desktop/launcher/src-tauri/tauri.conf.json',
      'packages/tokens/src/index.ts',
    ]) {
      expect(await exists(root, kept)).toBe(true);
    }
  });

  test('clean_deep_removes_node_modules_and_keep_target_leaves_target', async () => {
    const root = await fixture();
    await removePaths(await planClean(root, { deep: true, keepTarget: true }));

    expect(await exists(root, 'node_modules')).toBe(false);
    expect(await exists(root, 'programs/webapp/example/node_modules')).toBe(false);
    expect(await exists(root, 'target/debug/build.log')).toBe(true);
    expect(await exists(root, 'release/.archive/slatecore-launcher-0.0.9-base.zip')).toBe(true);
  });

  test('clean_dry_run_lists_without_removing', async () => {
    const child = Bun.spawn([process.execPath, 'scripts/commands/clean.ts', '--dry-run'], {
      cwd: ROOT,
      stdout: 'pipe',
      stderr: 'pipe',
      env: { ...process.env, NO_COLOR: '1' },
    });
    const [out, code] = await Promise.all([new Response(child.stdout).text(), child.exited]);
    expect(code).toBe(0);
    expect(out).toContain('would be removed');
    expect(await Bun.file(join(ROOT, 'package.json')).exists()).toBe(true);
  });
});

describe('version', () => {
  const CARGO = `[workspace]
members = ["crates/paths"]

[workspace.package]
authors = ["GENSLATE"]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
serde = { version = "1.0.229" }
`;

  test('nextVersion_applies_patch_minor_major_and_explicit_versions', () => {
    expect(nextVersion('0.1.0', 'patch')).toBe('0.1.1');
    expect(nextVersion('0.1.9', 'minor')).toBe('0.2.0');
    expect(nextVersion('0.9.9', 'major')).toBe('1.0.0');
    expect(nextVersion('0.1.0', '2.3.4')).toBe('2.3.4');
    expect(() => nextVersion('0.1.0', 'huge')).toThrow('invalid bump');
  });

  test('cargo_version_edit_ignores_dependency_versions_and_arrays_before_it', () => {
    const out = setCargoWorkspaceVersion(CARGO, '0.2.0');
    expect(out).toContain('authors = ["GENSLATE"]\nversion = "0.2.0"');
    expect(out).toContain('serde = { version = "1.0.229" }');
  });

  test('version_bumps_package_cargo_and_tauri_conf', async () => {
    const root = await tempDir();
    await write(root, 'package.json', '{\n  "name": "slatecore",\n  "version": "0.1.0"\n}\n');
    await write(
      root,
      'packages/tokens/package.json',
      '{ "name": "@genslate/tokens", "version": "0.1.0" }\n',
    );
    await write(
      root,
      'programs/desktop/launcher/package.json',
      '{ "name": "@genslate/launcher", "version": "0.1.0" }\n',
    );
    await write(
      root,
      'programs/webapp/example/package.json',
      '{ "name": "@genslate/example", "version": "0.1.0" }\n',
    );
    await write(root, 'Cargo.toml', CARGO);
    await write(
      root,
      'programs/desktop/launcher/src-tauri/tauri.conf.json',
      '{ "productName": "SLATECORE LAUNCHER", "version": "0.1.0" }\n',
    );
    await write(
      root,
      'programs/desktop/follower/src-tauri/tauri.conf.json',
      '{ "productName": "SLATECORE Follower", "version": "../package.json" }\n',
    );

    const plan = await planVersionBump(root, 'minor');
    expect(plan.current).toBe('0.1.0');
    expect(plan.next).toBe('0.2.0');
    await writeVersionEdits(root, plan.edits);

    const text = (file: string) => Bun.file(join(root, file)).text();
    for (const file of [
      'package.json',
      'packages/tokens/package.json',
      'programs/desktop/launcher/package.json',
      'programs/webapp/example/package.json',
    ]) {
      expect((JSON.parse(await text(file)) as { version: string }).version).toBe('0.2.0');
    }
    expect(await text('Cargo.toml')).toContain('version = "0.2.0"');
    expect(await text('Cargo.toml')).toContain('serde = { version = "1.0.229" }');
    expect(await text('programs/desktop/launcher/src-tauri/tauri.conf.json')).toContain(
      '"version": "0.2.0"',
    );
    expect(await text('programs/desktop/follower/src-tauri/tauri.conf.json')).toContain(
      '"version": "../package.json"',
    );
  });

  test('version_dry_run_changes_nothing', async () => {
    const root = await tempDir();
    await write(root, 'package.json', '{ "name": "slatecore", "version": "0.1.0" }\n');
    const plan = await planVersionBump(root, 'patch');
    expect(plan.edits.map((edit) => edit.file)).toEqual(['package.json']);
    expect(await Bun.file(join(root, 'package.json')).text()).toContain('"version": "0.1.0"');
  });
});

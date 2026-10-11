/**
 * `bun run package`: the portable zips. Everything except the real exe (a few fake bytes stand in
 * for it) and Microsoft's cab (an injected extractor stands in for `expand.exe`) runs for real
 * against the repo's seeds, in temp folders.
 */
import { afterEach, describe, expect, test } from 'bun:test';
import { cp, mkdir, mkdtemp, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

import { extractZip, readZip, safeEntryPath, sha256File, writeZip } from '../lib/archive';
import { ROOT } from '../lib/paths';
import {
  archiveOldBuilds,
  assertLockResolved,
  buildRelease,
  licenseGaps,
  looksLikeWindowsExe,
  parseLock,
  readVersion,
  type Webview2Lock,
} from '../lib/release';
import { assertCleanEntries, planBase } from '../lib/stage';
import {
  BUILD_ARGV,
  baseZipName,
  DEFAULT_SETTINGS,
  EXE_ZIP_PATH,
  GENSLATE_LICENSE,
  LOCK_FILE,
  runtimeZipName,
  SEED_APP_DIR,
  systemExe,
} from '../lib/targets';

const temporary: string[] = [];

async function tempDir(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), 'slatecore-package-'));
  temporary.push(dir);
  return dir;
}

afterEach(async () => {
  for (const dir of temporary.splice(0)) await rm(dir, { recursive: true, force: true });
});

const FAKE_EXE = 'MZ fake slatecore-launcher.exe, a few bytes\n';

async function fakeExe(dir: string): Promise<string> {
  const path = join(dir, 'slatecore-launcher.exe');
  await writeFile(path, FAKE_EXE);
  return path;
}

async function write(root: string, file: string, text = 'x'): Promise<void> {
  const path = join(root, file);
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, text);
}

const LAUNCHER = 'programs/desktop/launcher';
const GENSLATE_LICENSE_SEED = `${SEED_APP_DIR}/licenses/genslate.LICENSE.md`;

/** A checkout with only what the packager reads: the real seeds, copied. */
async function seedRepo(): Promise<string> {
  const repo = await tempDir();
  for (const file of ['package.json', `${LAUNCHER}/package.json`]) {
    await mkdir(dirname(join(repo, file)), { recursive: true });
    await cp(join(ROOT, file), join(repo, file));
  }
  await mkdir(join(repo, LAUNCHER, 'src-tauri'), { recursive: true });
  await cp(
    join(ROOT, LAUNCHER, 'src-tauri', 'tauri.conf.json'),
    join(repo, LAUNCHER, 'src-tauri', 'tauri.conf.json'),
  );
  for (const dir of ['installDir', 'other', 'README.md']) {
    const from = join(ROOT, LAUNCHER, dir);
    if (await stat(from).catch(() => undefined)) {
      await cp(from, join(repo, LAUNCHER, dir), { recursive: true });
    }
  }
  return repo;
}

/** Everything a developer's checkout can hold next to the seeds and must never be shipped. */
const JUNK = [
  `${LAUNCHER}/installDir/storage/vault/vault.a.gvh`,
  `${LAUNCHER}/installDir/storage/vault/files/0123.gvf`,
  `${LAUNCHER}/installDir/storage/users/shared/Documents/note.txt`,
  `${LAUNCHER}/installDir/programs/portableapps.com/App/app.exe`,
  `${LAUNCHER}/installDir/programs/portapps.io/brave/brave.exe`,
  `${LAUNCHER}/installDir/programs/genslate/launcher/slatecore-launcher.exe`,
  `${LAUNCHER}/installDir/programs/genslate/launcher/extra.dll`,
  `${LAUNCHER}/other/launcher/cache/webview2/EBWebView/Local State`,
  `${LAUNCHER}/other/launcher/cache/tmp/scratch.tmp`,
  `${LAUNCHER}/other/launcher/cache/vault-session/0a1b2c3d/secret.txt`,
  `${LAUNCHER}/other/launcher/logs/launcher.log`,
  `${LAUNCHER}/other/launcher/database/recent.toml`,
  `${LAUNCHER}/other/launcher/configs/keybindings.toml`,
  `${LAUNCHER}/other/launcher/configs/metadata/genslate.toml`,
  `${LAUNCHER}/other/launcher/resources/screenshots/launcher-polar-night-home.png`,
  `${LAUNCHER}/other/launcher/documents/qa/notes.md`,
  `${LAUNCHER}/node_modules/left-pad/index.js`,
  `${LAUNCHER}/dist/index.html`,
  `${LAUNCHER}/src-tauri/gen/schemas/desktop-schema.json`,
  `${LAUNCHER}/.env`,
  `${LAUNCHER}/.genslate/secret.txt`,
  '.env.local',
  '.genslate/secret.txt',
  'target/release/slatecore-launcher.exe',
  'node_modules/x/index.js',
  '.turbo/cache/a.tar.zst',
  '.claude/project/screenshots/launcher/launcher-polar-night-home.png',
];

const REQUIRED_PLACEHOLDERS = [
  'other/launcher/cache/.gitkeep',
  'other/launcher/logs/.gitkeep',
  'other/launcher/database/.gitkeep',
  'storage/users/shared/Desktop/.gitkeep',
  'storage/users/shared/Documents/.gitkeep',
  'storage/users/shared/Downloads/.gitkeep',
  'storage/users/shared/Music/.gitkeep',
  'storage/users/shared/Pictures/.gitkeep',
  'storage/users/shared/Videos/.gitkeep',
  'programs/portableapps.com/.gitkeep',
  'programs/portapps.io/.gitkeep',
];

async function buildBase(repo: string, version = '1.2.3'): Promise<Record<string, Uint8Array>> {
  const out = await tempDir();
  const exe = await fakeExe(out);
  const zip = join(out, baseZipName(version));
  await writeZip(await planBase({ root: repo, exe, version }), zip);
  return readZip(zip);
}

const text = (bytes: Uint8Array | undefined): string =>
  new TextDecoder().decode(bytes ?? new Uint8Array());

describe('portable base zip', () => {
  test('package_zip_has_required_empty_dirs', async () => {
    const entries = await buildBase(ROOT);
    // Zip tools drop empty folders, so every empty folder is a zero byte placeholder file.
    for (const path of REQUIRED_PLACEHOLDERS) {
      expect(Object.keys(entries)).toContain(path);
      expect(entries[path]?.length).toBe(0);
    }
    expect(entries[EXE_ZIP_PATH]).toEqual(new TextEncoder().encode(FAKE_EXE));
    expect(EXE_ZIP_PATH).toBe('programs/genslate/launcher/slatecore-launcher.exe');
    // The vault folder is the exception: the vault counts every file in it as a plain file
    // waiting to be imported, so it is an explicit directory entry, never a placeholder.
    expect(Object.keys(entries)).toContain('storage/vault/');
    expect(Object.keys(entries).filter((path) => path.startsWith('storage/vault/'))).toEqual([
      'storage/vault/',
    ]);
  });

  test('package_zip_ships_the_seeds_the_launcher_expects', async () => {
    const entries = await buildBase(ROOT, '4.5.6');
    const names = Object.keys(entries);
    expect(names).toContain('other/launcher/configs/settings.toml');
    expect(names.filter((name) => name.startsWith('other/launcher/resources/icons/'))).not.toEqual(
      [],
    );
    // The launcher's own metadata is stamped with the version being packaged.
    const meta = Bun.TOML.parse(text(entries['other/launcher/configs/metadata/launcher.toml'])) as {
      build: { version: string; identifier: string };
      exe: { windows: string };
    };
    expect(meta.build.version).toBe('4.5.6');
    expect(meta.build.identifier).toBe('xyz.genslate.slatecore.launcher');
    expect(meta.exe.windows).toBe('slatecore-launcher.exe');
    // An empty seed document is a placeholder for future text and is not shipped.
    for (const name of names.filter((n) => /^other\/launcher\/(documents|licenses)\//.test(n))) {
      expect(entries[name]?.length).toBeGreaterThan(0);
    }
  });

  test('package_never_contains_runtime_state_or_generated_dirs', async () => {
    const clean = await seedRepo();
    const dirty = await seedRepo();
    for (const junk of JUNK) await write(dirty, junk, 'secret');
    // The developer's own settings (dev mode writes them into the seed) must not be shipped.
    await write(dirty, `${SEED_APP_DIR}/configs/settings.toml`, '[appearance]\ntheme = "mine"\n');

    const cleanEntries = Object.keys(await buildBase(clean)).sort();
    const dirtyZip = await buildBase(dirty);
    const dirtyEntries = Object.keys(dirtyZip).sort();
    expect(dirtyEntries).toEqual(cleanEntries);
    expect(text(dirtyZip['other/launcher/configs/settings.toml'])).toBe(DEFAULT_SETTINGS);

    const forbidden =
      /(^|\/)(target|node_modules|dist|gen|\.git|\.genslate|\.turbo|screenshots|ebwebview|vault-session|qa)(\/|$)|(^|\/)\.env|\.(log|tmp|pml|gvh|gvf|gvi)$/i;
    expect(cleanEntries.filter((entry) => forbidden.test(entry))).toEqual([]);
    for (const entry of cleanEntries) {
      // Nothing from the three runtime folders but their placeholders.
      if (/^other\/launcher\/(cache|logs|database)\//.test(entry)) {
        expect(entry.endsWith('/.gitkeep')).toBe(true);
      }
      if (/^storage\//.test(entry)) {
        expect(entry.endsWith('/.gitkeep') || entry === 'storage/vault/').toBe(true);
      }
    }
  });

  test('package_refuses_entries_outside_the_allowlist', () => {
    const file = (path: string) => ({
      path,
      source: { kind: 'bytes' as const, bytes: new Uint8Array() },
    });
    for (const bad of [
      'storage/vault/vault.a.gvh',
      'storage/users/shared/Documents/note.txt',
      'other/launcher/logs/launcher.log',
      'other/launcher/cache/webview2/EBWebView/Local State',
      'other/launcher/configs/metadata/genslate.toml',
      'programs/portableapps.com/FirefoxPortable/FirefoxPortable.exe',
      'target/release/slatecore-launcher.exe',
      '.env',
      '../escape.txt',
      '/absolute.txt',
      'C:/drive.txt',
      'programs\\genslate\\launcher\\slatecore-launcher.exe',
    ]) {
      expect(() => assertCleanEntries([file(bad)], 'base'), bad).toThrow();
    }
    expect(() => assertCleanEntries([file(EXE_ZIP_PATH), file('README.md')], 'base')).not.toThrow();
  });

  test('package_readme_is_titled_slatecore_launcher_by_genslate_and_states_what_the_launcher_cannot_control', async () => {
    const entries = await buildBase(ROOT, '7.8.9');
    const readme = text(entries['README.md']);
    expect(readme.split('\n')[0]).toBe('# SLATECORE LAUNCHER by GENSLATE');
    expect(readme).toContain('7.8.9');
    expect(readme).not.toContain('{{');
    // What stays outside the launcher's control (the About screen says the same).
    for (const limit of [
      'Recent files',
      'Prefetch',
      'notification-area',
      'PortableApps.com',
      'portapps.io',
      'WebView2',
    ]) {
      expect(readme, limit).toContain(limit);
    }
    expect(readme).toMatch(/cannot control/i);
    // No personal data, no absolute local paths.
    expect(readme).not.toMatch(/[A-Za-z]:\\|\/home\/|\/Users\//);
  });

  test('package_zip_paths_are_relative_forward_slashed_and_sorted', async () => {
    const names = Object.keys(await buildBase(ROOT));
    for (const name of names) {
      expect(name).not.toContain('\\');
      expect(name.startsWith('/')).toBe(false);
      expect(name).not.toMatch(/(^|\/)\.\.(\/|$)|:/);
    }
    expect(names).toEqual([...names].sort());
  });

  test('package_is_reproducible_for_the_same_inputs', async () => {
    const [a, b] = await Promise.all([tempDir(), tempDir()]);
    const exe = await fakeExe(await tempDir());
    await writeZip(await planBase({ root: ROOT, exe, version: '1.0.0' }), join(a, 'x.zip'));
    await writeZip(await planBase({ root: ROOT, exe, version: '1.0.0' }), join(b, 'x.zip'));
    expect(await sha256File(join(a, 'x.zip'))).toBe(await sha256File(join(b, 'x.zip')));
  });

  test('package_ships_the_default_settings_whatever_the_working_tree_holds', async () => {
    const entries = await buildBase(ROOT);
    expect(text(entries['other/launcher/configs/settings.toml'])).toBe(DEFAULT_SETTINGS);
    // The default is comments only: valid TOML with no keys, which the launcher reads as defaults.
    expect(Bun.TOML.parse(DEFAULT_SETTINGS)).toEqual({});
  });

  test('package_zip_carries_the_third_party_notice_with_the_real_license_texts', async () => {
    const entries = await buildBase(ROOT);
    const notice = text(entries['other/launcher/licenses/third-party.NOTICE.md']);
    expect(notice).toContain('SIL OPEN FONT LICENSE Version 1.1');
    expect(notice).toContain('The Inter Project Authors');
    expect(notice).toContain('The JetBrains Mono Project Authors');
    expect(notice).toContain('Attribution 4.0 International');
    expect(notice).toContain('Copyright (c) Microsoft Corporation');
    // The runtime's own notice belongs to the add-on.
    expect(Object.keys(entries)).not.toContain('other/launcher/licenses/webview2.NOTICE.md');
  });

  test('package_does_not_ship_maintainer_documents_to_end_users', async () => {
    const names = Object.keys(await buildBase(ROOT));
    expect(names).not.toContain('other/launcher/documents/release.md');
    expect(names).not.toContain('other/launcher/documents/portability.md');
  });

  test('package_refuses_a_missing_or_empty_exe', async () => {
    const dir = await tempDir();
    await expect(
      planBase({ root: ROOT, exe: join(dir, 'missing.exe'), version: '1.0.0' }),
    ).rejects.toThrow(/missing\.exe/);
    await writeFile(join(dir, 'empty.exe'), '');
    await expect(
      planBase({ root: ROOT, exe: join(dir, 'empty.exe'), version: '1.0.0' }),
    ).rejects.toThrow(/empty/);
  });

  test('package_seed_folders_hold_every_required_placeholder', async () => {
    const install = join(ROOT, LAUNCHER, 'installDir');
    const expected = REQUIRED_PLACEHOLDERS.filter((path) => !path.startsWith('other/'));
    for (const path of expected) {
      expect(await Bun.file(join(install, path)).exists(), path).toBe(true);
    }
    const other = join(ROOT, LAUNCHER, 'other', 'launcher');
    for (const dir of ['cache', 'logs', 'database']) {
      expect(await Bun.file(join(other, dir, '.gitkeep')).exists(), dir).toBe(true);
    }
  });

  test('shipped_settings_seed_is_valid_toml_without_absolute_paths', async () => {
    const seed = await readFile(
      join(ROOT, LAUNCHER, 'other/launcher/configs/settings.toml'),
      'utf8',
    );
    // An empty or comment-only file is the defaults: the launcher's loader treats a missing
    // key as its default (crates/launcher-core config tests).
    expect(() => Bun.TOML.parse(seed)).not.toThrow();
    expect(seed).not.toMatch(/[A-Za-z]:[\\/]|\/home\/|\/Users\//);
  });
});

describe('zip files', () => {
  test('extract_refuses_paths_that_escape_the_folder', async () => {
    for (const bad of ['../x', 'a/../../x', '/abs', 'C:/x', 'a\\b', 'a//b', '']) {
      expect(() => safeEntryPath(bad), bad).toThrow();
    }
    expect(safeEntryPath('a/b.txt')).toBe('a/b.txt');
    expect(safeEntryPath('a/')).toBe('a/');
  });

  test('zip_round_trips_files_and_directory_entries', async () => {
    const dir = await tempDir();
    const zip = join(dir, 'round.zip');
    await writeZip(
      [
        { path: 'a/', source: { kind: 'dir' } },
        { path: 'a/b.txt', source: { kind: 'bytes', bytes: new TextEncoder().encode('hello') } },
        { path: 'a/empty', source: { kind: 'bytes', bytes: new Uint8Array() } },
      ],
      zip,
    );
    const into = join(dir, 'out');
    await extractZip(zip, into);
    expect(await readFile(join(into, 'a', 'b.txt'), 'utf8')).toBe('hello');
    expect((await stat(join(into, 'a', 'empty'))).size).toBe(0);
    expect((await readdir(join(into, 'a'))).sort()).toEqual(['b.txt', 'empty']);
  });
});

describe('releases', () => {
  test('release_version_comes_from_the_repo', async () => {
    const pkg = (await Bun.file(join(ROOT, 'package.json')).json()) as { version: string };
    expect(await readVersion(ROOT)).toBe(pkg.version);
    const repo = await tempDir();
    await write(repo, 'package.json', JSON.stringify({ version: 'not-a-version' }));
    await expect(readVersion(repo)).rejects.toThrow(/semver/);
  });

  test('release_version_must_match_the_version_the_exe_embeds', async () => {
    const repo = await tempDir();
    await write(repo, 'package.json', JSON.stringify({ version: '1.2.3' }));
    await write(
      repo,
      'programs/desktop/launcher/package.json',
      JSON.stringify({ version: '1.2.4' }),
    );
    await expect(readVersion(repo)).rejects.toThrow(/launcher/);
    await write(
      repo,
      'programs/desktop/launcher/package.json',
      JSON.stringify({ version: '1.2.3' }),
    );
    expect(await readVersion(repo)).toBe('1.2.3');
  });

  test('license_gaps_name_an_empty_genslate_license_and_a_release_build_refuses_it', async () => {
    // The owner has not supplied the GENSLATE license text yet: the seed is empty.
    expect(await licenseGaps(ROOT)).toEqual([GENSLATE_LICENSE]);
    const exe = await fakeExe(await tempDir());

    // An ordinary build warns and goes on.
    const releaseDir = await tempDir();
    const built = await buildRelease({ root: ROOT, releaseDir, exe, version: '0.1.0' });
    expect(built.warnings.join('\n')).toContain('genslate.LICENSE.md');

    // A release build refuses before writing anything.
    const strict = await tempDir();
    await expect(
      buildRelease({ root: ROOT, releaseDir: strict, exe, version: '0.1.0', release: true }),
    ).rejects.toThrow(/genslate\.LICENSE\.md/);
    expect(await readdir(strict)).toEqual([]);

    // With the text supplied, both modes are quiet and a release build goes through.
    const repo = await seedRepo();
    await write(repo, GENSLATE_LICENSE_SEED, 'The license text, supplied by the owner.');
    expect(await licenseGaps(repo)).toEqual([]);
    const done = await buildRelease({
      root: repo,
      releaseDir: strict,
      exe,
      version: '0.1.0',
      release: true,
    });
    expect(done.warnings).toEqual([]);
    const entries = await readZip(done.base);
    expect(text(entries[GENSLATE_LICENSE])).toBe('The license text, supplied by the owner.');
  });

  test('a_failed_move_keeps_the_new_build_and_says_where_it_is', async () => {
    const releaseDir = await tempDir();
    const exe = await fakeExe(await tempDir());
    await write(releaseDir, 'slatecore-launcher-0.0.9-base.zip', 'old base');
    // `.archive` is a file, so the older build cannot be moved after the new one was written.
    await write(releaseDir, '.archive', 'in the way');
    let message = '';
    try {
      await buildRelease({ root: ROOT, releaseDir, exe, version: '0.1.0' });
    } catch (error) {
      message = error instanceof Error ? error.message : String(error);
    }
    expect(message).toContain('.building');
    expect(message).toContain('slatecore-launcher-0.1.0-base.zip');
    expect(await Bun.file(join(releaseDir, '.building', baseZipName('0.1.0'))).exists()).toBe(true);
    expect(await readFile(join(releaseDir, 'slatecore-launcher-0.0.9-base.zip'), 'utf8')).toBe(
      'old base',
    );
  });

  test('the_exe_check_wants_an_mz_header_and_system_tools_need_systemroot', async () => {
    const dir = await tempDir();
    await writeFile(join(dir, 'real.exe'), 'MZ....');
    await writeFile(join(dir, 'fake.exe'), 'hello');
    expect(await looksLikeWindowsExe(join(dir, 'real.exe'))).toBe(true);
    expect(await looksLikeWindowsExe(join(dir, 'fake.exe'))).toBe(false);
    expect(systemExe('expand.exe', { SystemRoot: 'D:\\Win' })).toBe(
      'D:\\Win\\System32\\expand.exe',
    );
    // No guess at C:\Windows.
    expect(() => systemExe('expand.exe', {})).toThrow(/SystemRoot/);
  });

  test('old_builds_move_to_archive', async () => {
    const releaseDir = await tempDir();
    const exe = await fakeExe(await tempDir());
    await write(releaseDir, 'slatecore-launcher-0.0.9-base.zip', 'old base');
    await write(releaseDir, 'slatecore-launcher-0.0.9-webview2-runtime.zip', 'old runtime');
    await write(releaseDir, '.gitkeep', '');
    await write(releaseDir, 'notes.txt', 'keep me');

    const first = await buildRelease({ root: ROOT, releaseDir, exe, version: '0.1.0' });
    expect(first.base).toBe(join(releaseDir, 'slatecore-launcher-0.1.0-base.zip'));
    expect(first.archived.map((path) => path.split(/[\\/]/).pop()).sort()).toEqual([
      'slatecore-launcher-0.0.9-base.zip',
      'slatecore-launcher-0.0.9-webview2-runtime.zip',
    ]);
    expect((await readdir(releaseDir)).sort()).toEqual([
      '.archive',
      '.gitkeep',
      'notes.txt',
      'slatecore-launcher-0.1.0-base.zip',
    ]);
    expect((await readdir(join(releaseDir, '.archive'))).sort()).toEqual([
      'slatecore-launcher-0.0.9-base.zip',
      'slatecore-launcher-0.0.9-webview2-runtime.zip',
    ]);
    expect(
      await readFile(join(releaseDir, '.archive', 'slatecore-launcher-0.0.9-base.zip'), 'utf8'),
    ).toBe('old base');

    // Packaging the same version again archives the previous build under another name.
    await buildRelease({ root: ROOT, releaseDir, exe, version: '0.1.0' });
    const archived = await readdir(join(releaseDir, '.archive'));
    expect(archived).toHaveLength(3);
    expect(
      archived.filter((name) => name.startsWith('slatecore-launcher-0.1.0-base')),
    ).toHaveLength(1);
    expect((await readdir(releaseDir)).filter((name) => name.endsWith('.zip'))).toEqual([
      'slatecore-launcher-0.1.0-base.zip',
    ]);
  });

  test('a_failed_package_leaves_the_existing_builds_alone', async () => {
    const releaseDir = await tempDir();
    await write(releaseDir, 'slatecore-launcher-0.0.9-base.zip', 'old base');
    await expect(
      buildRelease({ root: ROOT, releaseDir, exe: join(releaseDir, 'nope.exe'), version: '0.1.0' }),
    ).rejects.toThrow();
    expect((await readdir(releaseDir)).sort()).toEqual(['slatecore-launcher-0.0.9-base.zip']);
  });

  test('archive_only_touches_launcher_zips', async () => {
    const releaseDir = await tempDir();
    await write(releaseDir, 'other-tool-1.0.0.zip');
    await write(releaseDir, 'slatecore-launcher-1.0.0-base.zip');
    const moved = await archiveOldBuilds(releaseDir);
    expect(moved).toHaveLength(1);
    expect(await readdir(releaseDir)).toContain('other-tool-1.0.0.zip');
  });
});

describe('build step', () => {
  test('build_uses_tauri_no_bundle_and_never_an_installer_mode', async () => {
    expect(BUILD_ARGV).toEqual(['bun', 'x', '--no-install', 'tauri', 'build', '--no-bundle']);
    const source = await readFile(join(ROOT, 'scripts/commands/package.ts'), 'utf8');
    const lib = await readFile(join(ROOT, 'scripts/lib/targets.ts'), 'utf8');
    for (const mode of ['--bundles', 'nsis', 'wix', 'msi']) {
      expect(source.toLowerCase(), mode).not.toContain(mode);
      expect(lib.toLowerCase(), mode).not.toContain(mode);
    }
  });
});

/** A lock a Windows machine would commit after downloading the cab. */
const RESOLVED: Webview2Lock = {
  schema: 1,
  version: '154.0.4258.62',
  architecture: 'x64',
  fileName: 'Microsoft.WebView2.FixedVersionRuntime.154.0.4258.62.x64.cab',
  url: 'https://msedge.sf.dl.delivery.mp.microsoft.com/filestreamingservice/files/example/Microsoft.WebView2.FixedVersionRuntime.154.0.4258.62.x64.cab',
  sha256: 'a'.repeat(64),
  size: 123,
  licenseNotice: 'programs/desktop/launcher/other/launcher/licenses/webview2.NOTICE.md',
};

describe('webview2 runtime add-on', () => {
  test('webview2_lock_requires_hash', async () => {
    // The lock that ships in the repo has no hash yet, so the add-on step refuses to run.
    const shipped = parseLock(await readFile(join(ROOT, LOCK_FILE), 'utf8'));
    expect(() => assertLockResolved(shipped)).toThrow(/unresolved/i);
    expect(() => assertLockResolved(shipped)).toThrow(/sha256/);

    expect(() => assertLockResolved(RESOLVED)).not.toThrow();
    for (const sha256 of [
      '',
      'UNRESOLVED',
      '0'.repeat(64),
      'A'.repeat(64),
      'abc',
      'g'.repeat(64),
    ]) {
      expect(() => assertLockResolved({ ...RESOLVED, sha256 }), sha256).toThrow(/sha256/);
    }
    expect(() => assertLockResolved({ ...RESOLVED, size: 0 })).toThrow(/size/);
    expect(() => assertLockResolved({ ...RESOLVED, version: '154' })).toThrow(/version/);
    expect(() => assertLockResolved({ ...RESOLVED, fileName: 'UNRESOLVED' })).toThrow(/fileName/);
  });

  test('webview2_lock_accepts_only_microsoft_sources', () => {
    for (const url of [
      'http://msedge.sf.dl.delivery.mp.microsoft.com/a.cab',
      'https://example.com/a.cab',
      'https://microsoft.com.evil.example/a.cab',
      'https://evilmicrosoft.com/a.cab',
      'https://user@download.microsoft.com/a.cab',
      'ftp://download.microsoft.com/a.cab',
      'not a url',
      'UNRESOLVED',
    ]) {
      expect(() => assertLockResolved({ ...RESOLVED, url }), url).toThrow(/url|Microsoft/);
    }
    for (const url of [
      'https://download.microsoft.com/download/x/a.cab',
      'https://msedge.sf.dl.delivery.mp.microsoft.com/a.cab',
      'https://go.microsoft.com/fwlink/?linkid=1',
    ]) {
      expect(() => assertLockResolved({ ...RESOLVED, url }), url).not.toThrow();
    }
  });

  test('webview2_lock_file_is_well_formed_json_with_the_documented_fields', async () => {
    const raw = JSON.parse(await readFile(join(ROOT, LOCK_FILE), 'utf8')) as Record<
      string,
      unknown
    >;
    for (const key of [
      'schema',
      'version',
      'architecture',
      'fileName',
      'url',
      'sha256',
      'size',
      'licenseNotice',
    ]) {
      expect(raw, key).toHaveProperty(key);
    }
    const notice = raw['licenseNotice'];
    expect(typeof notice).toBe('string');
    expect(await Bun.file(join(ROOT, String(notice))).exists()).toBe(true);
  });

  /** Writes a fake cab whose lock entry matches it, and the extractor that "expands" it. */
  async function fakeCab(): Promise<{
    cab: string;
    lock: Webview2Lock;
    extract: (cab: string, to: string) => Promise<void>;
  }> {
    const dir = await tempDir();
    const cab = join(dir, RESOLVED.fileName);
    await writeFile(cab, 'pretend this is a cab');
    const lock: Webview2Lock = {
      ...RESOLVED,
      sha256: await sha256File(cab),
      size: (await stat(cab)).size,
    };
    const extract = async (_cab: string, to: string): Promise<void> => {
      const runtime = join(to, `Microsoft.WebView2.FixedVersionRuntime.${lock.version}.x64`);
      await write(runtime, 'msedgewebview2.exe', 'MZ runtime');
      await write(runtime, 'Locales/en-US.pak', 'pak');
    };
    return { cab, lock, extract };
  }

  test('webview2_runtime_zip_unpacks_to_other_shared_webview2_version_x64', async () => {
    const { cab, lock, extract } = await fakeCab();
    const releaseDir = await tempDir();
    const exe = await fakeExe(await tempDir());
    const result = await buildRelease({
      root: ROOT,
      releaseDir,
      exe,
      version: '0.1.0',
      webview2: { cab, lock, extract },
      workDir: await tempDir(),
    });
    expect(result.runtime).toBe(join(releaseDir, runtimeZipName('0.1.0')));
    const entries = await readZip(result.runtime ?? '');
    const names = Object.keys(entries).sort();
    const base = `other/shared/webview2/${lock.version}-x64`;
    expect(names).toContain(`${base}/msedgewebview2.exe`);
    expect(names).toContain(`${base}/Locales/en-US.pak`);
    expect(names).toContain('other/launcher/licenses/webview2.NOTICE.md');
    // Only the add-on's own files, nothing of the base package.
    expect(names.every((name) => name.startsWith('other/'))).toBe(true);
    const info = JSON.parse(text(entries[`${base}/runtime.json`])) as Record<string, unknown>;
    expect(info).toMatchObject({ version: lock.version, sha256: lock.sha256, source: lock.url });
    // The base zip is built next to it and does not carry the runtime or its notice.
    const baseEntries = await readZip(result.base);
    expect(Object.keys(baseEntries).some((name) => name.includes('webview2/'))).toBe(false);
    expect(Object.keys(baseEntries)).not.toContain('other/launcher/licenses/webview2.NOTICE.md');
  });

  test('webview2_runtime_refuses_a_cab_that_does_not_match_the_lock', async () => {
    const { cab, lock, extract } = await fakeCab();
    const releaseDir = await tempDir();
    const exe = await fakeExe(await tempDir());
    await expect(
      buildRelease({
        root: ROOT,
        releaseDir,
        exe,
        version: '0.1.0',
        webview2: { cab, lock: { ...lock, sha256: 'b'.repeat(64) }, extract },
      }),
    ).rejects.toThrow(/sha256/);
    await expect(
      buildRelease({
        root: ROOT,
        releaseDir,
        exe,
        version: '0.1.0',
        webview2: { cab, lock: { ...lock, size: lock.size + 1 }, extract },
      }),
    ).rejects.toThrow(/size/);
    // Nothing was written: the check happens before any zip.
    expect(await readdir(releaseDir)).toEqual([]);
  });

  test('webview2_runtime_refuses_an_extracted_folder_without_msedgewebview2', async () => {
    const { cab, lock } = await fakeCab();
    const releaseDir = await tempDir();
    const exe = await fakeExe(await tempDir());
    const extract = async (_cab: string, to: string): Promise<void> => {
      await write(join(to, 'Microsoft.WebView2.FixedVersionRuntime.154.0.4258.62.x64'), 'x.txt');
    };
    await expect(
      buildRelease({
        root: ROOT,
        releaseDir,
        exe,
        version: '0.1.0',
        webview2: { cab, lock, extract },
        workDir: await tempDir(),
      }),
    ).rejects.toThrow(/msedgewebview2\.exe/);
  });
});

describe('package command', () => {
  async function runCommand(
    args: readonly string[],
    extraEnv: Record<string, string> = {},
  ): Promise<{ code: number; out: string }> {
    // A tag build of the test job must not turn these runs into release builds.
    const { GITHUB_REF_TYPE: _tag, ...inherited } = process.env;
    const child = Bun.spawn(['bun', join(ROOT, 'scripts/commands/package.ts'), ...args], {
      cwd: ROOT,
      stdout: 'pipe',
      stderr: 'pipe',
      env: { ...inherited, NO_COLOR: '1', ...extraEnv },
    });
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    return { code, out: stdout + stderr };
  }

  test('package_command_lists_its_options', async () => {
    const { code, out } = await runCommand(['--help']);
    expect(code).toBe(0);
    for (const option of ['--exe', '--no-build', '--webview2', '--out', '--release']) {
      expect(out).toContain(option);
    }
  });

  test('package_command_packages_an_existing_exe_without_building', async () => {
    const releaseDir = await tempDir();
    const exe = await fakeExe(await tempDir());
    const { code, out } = await runCommand(['--exe', exe, '--out', releaseDir]);
    expect(code, out).toBe(0);
    const version = await readVersion(ROOT);
    expect(await readdir(releaseDir)).toEqual([baseZipName(version)]);
    const entries = await readZip(join(releaseDir, baseZipName(version)));
    expect(entries[EXE_ZIP_PATH]).toEqual(new TextEncoder().encode(FAKE_EXE));
  });

  test('package_command_warns_loudly_about_the_empty_license_and_refuses_it_for_releases', async () => {
    const exe = await fakeExe(await tempDir());
    const ordinary = await runCommand(['--exe', exe, '--out', await tempDir()]);
    expect(ordinary.code, ordinary.out).toBe(0);
    expect(ordinary.out).toMatch(/WARNING/);
    expect(ordinary.out).toContain('genslate.LICENSE.md');

    for (const [args, env] of [
      [['--release'], {}],
      [[], { GITHUB_REF_TYPE: 'tag' }],
    ] as const) {
      const releaseDir = await tempDir();
      const run = await runCommand(['--exe', exe, '--out', releaseDir, ...args], env);
      expect(run.code, run.out).not.toBe(0);
      expect(run.out).toContain('genslate.LICENSE.md');
      expect(await readdir(releaseDir)).toEqual([]);
    }
    // Other ref types are ordinary builds.
    const branch = await runCommand(['--exe', exe, '--out', await tempDir()], {
      GITHUB_REF_TYPE: 'branch',
    });
    expect(branch.code, branch.out).toBe(0);
  });

  test('package_command_refuses_the_runtime_add_on_while_the_lock_has_no_hash', async () => {
    const releaseDir = await tempDir();
    const exe = await fakeExe(await tempDir());
    const cab = join(await tempDir(), 'some.cab');
    await writeFile(cab, 'cab');
    const { code, out } = await runCommand(['--exe', exe, '--out', releaseDir, '--webview2', cab]);
    expect(code).not.toBe(0);
    expect(out).toMatch(/unresolved/i);
    expect(out).toContain('webview2.lock.json');
    // The refusal comes before any work: not even the base zip is written.
    expect(await readdir(releaseDir)).toEqual([]);
  });

  test('package_command_reports_a_missing_exe', async () => {
    const { code, out } = await runCommand([
      '--exe',
      join(await tempDir(), 'nope.exe'),
      '--out',
      await tempDir(),
    ]);
    expect(code).not.toBe(0);
    expect(out).toContain('nope.exe');
  });

  test.skipIf(process.platform === 'win32')(
    'package_command_does_not_build_the_windows_app_on_another_host (skipped on Windows)',
    async () => {
      const releaseDir = await tempDir();
      const { code, out } = await runCommand(['--out', releaseDir]);
      expect(code).not.toBe(0);
      expect(out).toMatch(/Windows/);
      expect(out).toContain('--exe');
      expect(await readdir(releaseDir)).toEqual([]);
    },
  );
});

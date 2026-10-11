/**
 * The portability test, tiers 2 and 3 (tier 1 is `src-tauri/tests/no_trace.rs`, a cargo test).
 *
 * - The helpers (folder listing, registry export parsing, diffs, the Process Monitor CSV parser
 *   and the allowlist rules) are plain functions and always run, here and on Windows.
 * - The real tier 2 run (start the packaged exe, snapshot the OS folders and `HKCU\Software`
 *   around it, compare with a control run) needs Windows and a built zip. Elsewhere it is
 *   skipped, and says why in its name and on the console; it never passes without running.
 */
import { afterEach, describe, expect, test } from 'bun:test';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

import { extractZip } from '../lib/archive';
import { ROOT } from '../lib/paths';
import { readVersion } from '../lib/release';
import { baseZipName, EXE_ZIP_PATH } from '../lib/targets';
import {
  checkTrace,
  globToRegExp,
  isWrite,
  launcherProcessIds,
  type ProcmonRow,
  parseAllowlist,
  parseProcmonCsv,
  pmlToCsvArgs,
} from '../no-trace/procmon';
import {
  diffSnapshots,
  exportHkcuSoftware,
  filterIgnored,
  gatherFacts,
  type HostSnapshot,
  launcherEnv,
  leaksOf,
  listFolder,
  parseIgnoreList,
  parseRegExport,
  runControl,
  runLauncher,
  takeSnapshot,
  tier2SkipReason,
} from '../no-trace/tier2';

const temporary: string[] = [];

async function tempDir(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), 'slatecore-no-trace-'));
  temporary.push(dir);
  return dir;
}

afterEach(async () => {
  for (const dir of temporary.splice(0)) await rm(dir, { recursive: true, force: true });
});

async function write(root: string, file: string, text = 'x'): Promise<void> {
  const path = join(root, file);
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, text);
}

// ─── tier 2: helpers (run everywhere) ──────────────────────────────────────────────────────────

describe('tier 2 helpers', () => {
  test('list_folder_lists_entries_to_the_given_depth_and_marks_folders', async () => {
    const dir = await tempDir();
    await write(dir, 'a.txt');
    await write(dir, 'app/deep/file.bin');
    const names = async (depth: number) => Object.keys((await listFolder(dir, depth)) ?? {});
    expect(await names(1)).toEqual(['a.txt', 'app/']);
    expect(await names(2)).toEqual(['a.txt', 'app/', 'app/deep/']);
    expect(await names(3)).toEqual(['a.txt', 'app/', 'app/deep/', 'app/deep/file.bin']);
    // Folders have no signature; files carry size:mtime.
    const listing = (await listFolder(dir, 3)) ?? {};
    expect(listing['app/']).toBe('');
    expect(listing['a.txt']).toMatch(/^1:\d+$/);
    expect(await listFolder(join(dir, 'missing'), 1)).toBeNull();
  });

  test('reg_export_is_parsed_per_key_and_value_changes_are_seen', () => {
    const before = [
      'Windows Registry Editor Version 5.00',
      '',
      '[HKEY_CURRENT_USER\\Software\\Vendor]',
      '"Setting"=dword:00000001',
      '',
      '[HKEY_CURRENT_USER\\Software\\Vendor\\Child]',
      '@="x"',
      '"List"=hex:01,02,\\',
      '  03,04',
      '',
    ].join('\r\n');
    const keys = parseRegExport(before);
    expect(Object.keys(keys)).toEqual([
      'HKEY_CURRENT_USER\\Software\\Vendor',
      'HKEY_CURRENT_USER\\Software\\Vendor\\Child',
    ]);
    // A byte order mark and the line ending do not change anything.
    expect(parseRegExport(`\uFEFF${before.replaceAll('\r\n', '\n')}`)).toEqual(keys);
    // A changed value changes the key's hash.
    const changed = parseRegExport(before.replace('dword:00000001', 'dword:00000002'));
    expect(changed['HKEY_CURRENT_USER\\Software\\Vendor']).not.toBe(
      keys['HKEY_CURRENT_USER\\Software\\Vendor'],
    );
    expect(changed['HKEY_CURRENT_USER\\Software\\Vendor\\Child']).toBe(
      keys['HKEY_CURRENT_USER\\Software\\Vendor\\Child'],
    );
  });

  test('diff_reports_added_removed_and_changed_entries_in_folders_and_registry', () => {
    const before: HostSnapshot = {
      folders: { TEMP: { keep: '1:1', 'gone.tmp': '1:1', 'edited.cfg': '5:100' }, APPDATA: null },
      registry: { 'HKCU\\A': '1', 'HKCU\\B': '1', 'HKCU\\C': '1' },
    };
    const after: HostSnapshot = {
      folders: {
        TEMP: { keep: '1:1', 'new.tmp': '1:1', 'edited.cfg': '6:200' },
        APPDATA: { 'xyz.genslate/': '' },
      },
      registry: { 'HKCU\\A': '1', 'HKCU\\B': '2', 'HKCU\\D': '1' },
    };
    expect(diffSnapshots(before, after)).toEqual([
      '+ APPDATA (the folder itself)',
      '+ APPDATA/xyz.genslate/',
      '+ HKCU\\D',
      '+ TEMP/new.tmp',
      '- HKCU\\C',
      '- TEMP/gone.tmp',
      '~ HKCU\\B',
      // A file edited in place (same name, other size and time) is a change too.
      '~ TEMP/edited.cfg',
    ]);
    expect(diffSnapshots(before, before)).toEqual([]);
  });

  test('leaks_are_the_changes_the_control_run_did_not_also_see', () => {
    const control = ['+ TEMP/background.tmp', '~ HKCU\\Noise'];
    const app = ['+ TEMP/background.tmp', '+ LOCALAPPDATA/xyz.genslate.slatecore.launcher/'];
    expect(leaksOf(control, app)).toEqual(['+ LOCALAPPDATA/xyz.genslate.slatecore.launcher/']);
    expect(leaksOf(control, control)).toEqual([]);
    expect(leaksOf([], app)).toEqual(app);
  });

  test('take_snapshot_watches_only_the_folders_that_are_set', async () => {
    const dir = await tempDir();
    await write(dir, 'temp/a.tmp');
    const snapshot = await takeSnapshot(
      { TEMP: join(dir, 'temp'), APPDATA: undefined, LOCALAPPDATA: join(dir, 'none') },
      async () => '[HKCU\\X]\n"a"=1\n',
    );
    expect(Object.keys(snapshot.folders['TEMP'] ?? {})).toEqual(['a.tmp']);
    expect(snapshot.folders['LOCALAPPDATA']).toBeNull();
    expect(Object.keys(snapshot.folders)).toEqual(['TEMP', 'LOCALAPPDATA']);
    expect(Object.keys(snapshot.registry)).toEqual(['HKCU\\X']);
  });

  test('a_leak_is_found_when_a_folder_is_written_during_the_run', async () => {
    const dir = await tempDir();
    await write(dir, 'local/noise.tmp');
    const places = { LOCALAPPDATA: join(dir, 'local') };
    const registry = async (): Promise<string> => '';
    const start = await takeSnapshot(places, registry);
    await write(dir, 'local/xyz.genslate.slatecore.launcher/EBWebView/Local State');
    const end = await takeSnapshot(places, registry);
    const control = diffSnapshots(start, start);
    expect(leaksOf(control, diffSnapshots(start, end))).toEqual([
      '+ LOCALAPPDATA/xyz.genslate.slatecore.launcher/',
      '+ LOCALAPPDATA/xyz.genslate.slatecore.launcher/EBWebView/',
    ]);
  });

  test('an_existing_file_edited_in_place_shows_up_as_a_change', async () => {
    const dir = await tempDir();
    await write(dir, 'local/settings.dat', 'one');
    const places = { LOCALAPPDATA: join(dir, 'local') };
    const registry = async (): Promise<string> => '';
    const start = await takeSnapshot(places, registry);
    await write(dir, 'local/settings.dat', 'one and more');
    expect(diffSnapshots(start, await takeSnapshot(places, registry))).toEqual([
      '~ LOCALAPPDATA/settings.dat',
    ]);
  });

  test('ignore_rules_drop_windows_owned_noise_and_nothing_else', async () => {
    const list = parseIgnoreList(
      JSON.parse(await readFile(join(ROOT, 'scripts/no-trace/tier2-ignore.json'), 'utf8')),
    );
    const store =
      'HKEY_CURRENT_USER\\Software\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Compatibility Assistant\\Store';
    const changes = [
      `~ ${store}`,
      '+ HKEY_CURRENT_USER\\Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\CurrentVersion\\TrayNotify',
      '~ HKEY_CURRENT_USER\\Software\\classes\\local settings\\software\\microsoft\\windows\\shell\\muicache',
      '~ HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\UserAssist\\{GUID}\\Count',
      '+ LOCALAPPDATA/Microsoft/Windows/Explorer/iconcache_32.db',
      '+ LOCALAPPDATA/D3DSCache/',
      // The launcher's own leaks are never ignored.
      '+ LOCALAPPDATA/xyz.genslate.slatecore.launcher/',
      '+ HKEY_CURRENT_USER\\Software\\Microsoft\\Edge\\BLBeacon',
      '+ HKEY_CURRENT_USER\\Software\\GENSLATE',
      '+ TEMP/scratch.tmp',
      // Only below a rule, not a sibling that shares its start.
      '+ HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\UserAssistant',
    ];
    expect(filterIgnored(changes, list.rules)).toEqual(changes.slice(6));
  });

  test('the_shipped_ignore_list_is_valid_reasoned_and_marked_provisional', async () => {
    const list = parseIgnoreList(
      JSON.parse(await readFile(join(ROOT, 'scripts/no-trace/tier2-ignore.json'), 'utf8')),
    );
    // True until a real Windows run has confirmed it; change it only with that run.
    expect(list.provisional).toBe(true);
    for (const rule of list.rules) expect(rule.reason, rule.id).toMatch(/^Provisional/);
    const rule = {
      id: 'a',
      reason: 'a reason that is long enough',
      path: 'HKEY_CURRENT_USER\\Software\\X\\Y',
    };
    expect(() => parseIgnoreList({ schema: 1, provisional: true, rules: [rule] })).not.toThrow();
    expect(() => parseIgnoreList({ schema: 2, provisional: true, rules: [] })).toThrow(/schema/);
    expect(() => parseIgnoreList({ schema: 1, rules: [] })).toThrow(/provisional/);
    expect(() =>
      parseIgnoreList({ schema: 1, provisional: true, rules: [{ ...rule, reason: 'short' }] }),
    ).toThrow(/reason/);
    for (const path of [
      'HKEY_CURRENT_USER\\**',
      'HKEY_CURRENT_USER\\Software\\**',
      'LOCALAPPDATA/**',
      'C:/Users/x',
      'HKEY_LOCAL_MACHINE\\Software\\X\\Y',
    ]) {
      expect(
        () => parseIgnoreList({ schema: 1, provisional: true, rules: [{ ...rule, path }] }),
        path,
      ).toThrow();
    }
    expect(() => parseIgnoreList({ schema: 1, provisional: true, rules: [rule, rule] })).toThrow(
      /duplicate/,
    );
  });

  test('the_launcher_environment_loses_webview2_variables_only', () => {
    const env = launcherEnv({
      PATH: 'p',
      WEBVIEW2_USER_DATA_FOLDER: 'x',
      webview2_browser_executable_folder: 'y',
      Empty: undefined,
    });
    expect(env['PATH']).toBe('p');
    expect(Object.keys(env).some((name) => name.toUpperCase().startsWith('WEBVIEW2_'))).toBe(false);
    expect(env['Empty']).toBeUndefined();
  });
});

// ─── tier 2: when the real run is possible ─────────────────────────────────────────────────────

describe('tier 2 skip reasons', () => {
  const ready = {
    platform: 'win32',
    elevated: false,
    zipPath: 'release/x.zip',
    exeIsReal: true,
    disabled: false,
  } as const;

  test('tier2_runs_only_on_windows_with_a_built_zip_and_a_normal_shell', () => {
    expect(tier2SkipReason(ready)).toBeUndefined();
    expect(tier2SkipReason({ ...ready, platform: 'linux' })).toMatch(/Windows/);
    expect(tier2SkipReason({ ...ready, platform: 'darwin' })).toMatch(/Windows/);
    expect(tier2SkipReason({ ...ready, elevated: true })).toMatch(/elevated/);
    expect(tier2SkipReason({ ...ready, zipPath: undefined })).toMatch(/bun run package/);
    expect(tier2SkipReason({ ...ready, exeIsReal: false })).toMatch(/placeholder exe/);
    expect(tier2SkipReason({ ...ready, disabled: true })).toMatch(/SLATECORE_NO_TRACE=0/);
  });
});

/** The zip the packager wrote for this version, or the one named by `SLATECORE_NO_TRACE_ZIP`. */
async function findZip(): Promise<string | undefined> {
  const named = process.env['SLATECORE_NO_TRACE_ZIP'];
  const candidate = named ?? join(ROOT, 'release', baseZipName(await readVersion(ROOT)));
  return (await Bun.file(candidate).exists()) ? candidate : undefined;
}

const facts = await gatherFacts(await findZip());
const SKIP = tier2SkipReason(facts);
if (SKIP !== undefined) console.log(`[skipped] tier 2 (packaged exe vs a control run): ${SKIP}`);

describe('tier 2: the packaged exe leaves no trace', () => {
  test.skipIf(SKIP !== undefined)(
    `tier2_packaged_exe_leaves_no_trace${SKIP === undefined ? '' : ` (SKIPPED: ${SKIP})`}`,
    async () => {
      const zip = facts.zipPath ?? '';
      // The unpacked install folder lives under target/ (git ignored), never in a watched folder.
      const parent = join(ROOT, 'target', 'no-trace');
      await mkdir(parent, { recursive: true });
      const installDir = await mkdtemp(join(parent, 'run-'));
      temporary.push(installDir);
      // The registry export holds the whole HKCU\Software hive: it is deleted with the run.
      temporary.push(join(parent, 'registry'), join(parent, 'control'));
      await extractZip(zip, installDir);
      const exe = join(installDir, ...EXE_ZIP_PATH.split('/'));

      const places = {
        TEMP: process.env['TEMP'] ?? tmpdir(),
        APPDATA: process.env['APPDATA'],
        LOCALAPPDATA: process.env['LOCALAPPDATA'],
      };
      const snapshot = (): Promise<HostSnapshot> =>
        takeSnapshot(places, () => exportHkcuSoftware(join(installDir, '..', 'registry')));

      // 1. The launcher run: start, wait until it is ready, end it.
      const before = await snapshot();
      const started = Date.now();
      const run = await runLauncher({ exe, installDir, readyTimeoutMs: 90_000, settleMs: 5_000 });
      const afterRun = await snapshot();
      const runChanges = diffSnapshots(before, afterRun);

      // 2. The control run: a harmless exe (a copy of a System32 program) and the same wait, to
      // measure the PC's own noise, including what Windows writes for any new exe it sees.
      await runControl({
        workDir: join(installDir, '..', 'control'),
        durationMs: Math.max(0, Date.now() - started),
      });
      const afterControl = await snapshot();
      const controlChanges = diffSnapshots(afterRun, afterControl);
      const ignore = parseIgnoreList(
        JSON.parse(await readFile(join(ROOT, 'scripts/no-trace/tier2-ignore.json'), 'utf8')),
      );

      // It really started (not a vacuous pass), and it wrote its state inside the install folder.
      expect(run.ready, `the launcher never logged its start. Log tail:\n${run.logTail}`).toBe(
        true,
      );
      expect(await Bun.file(join(installDir, 'other/launcher/logs/launcher.log')).exists()).toBe(
        true,
      );
      expect(filterIgnored(leaksOf(controlChanges, runChanges), ignore.rules)).toEqual([]);
    },
    600_000,
  );
});

// ─── tier 3: Process Monitor trace against the allowlist ───────────────────────────────────────

const FIXTURE = join(ROOT, 'scripts/no-trace/fixtures/procmon-sample.csv');
const INSTALL = 'E:\\Test Install';
const ENV = {
  LOCALAPPDATA: 'C:\\Users\\Test\\AppData\\Local',
  ProgramData: 'C:\\ProgramData',
};

async function fixtureText(): Promise<string> {
  return (await readFile(FIXTURE, 'utf8'))
    .replaceAll('@INSTALL@', INSTALL)
    .replaceAll('@LOCALAPPDATA@', ENV.LOCALAPPDATA);
}

async function shippedAllowlist() {
  return parseAllowlist(
    JSON.parse(await readFile(join(ROOT, 'scripts/no-trace/allowlist.json'), 'utf8')),
  );
}

const row = (overrides: Partial<ProcmonRow>): ProcmonRow => ({
  time: '9:00:00 AM',
  process: 'slatecore-launcher.exe',
  pid: 1,
  operation: 'WriteFile',
  path: 'C:\\x',
  result: 'SUCCESS',
  detail: '',
  ...overrides,
});

describe('tier 3: Process Monitor trace', () => {
  test('procmon_csv_is_parsed_with_quotes_commas_and_a_byte_order_mark', async () => {
    const rows = parseProcmonCsv(`\uFEFF${await fixtureText()}`);
    expect(rows).toHaveLength(15);
    expect(rows[0]).toMatchObject({
      process: 'explorer.exe',
      pid: 1000,
      operation: 'Process Create',
    });
    // A doubled quote inside a quoted field is one quote.
    expect(rows[0]?.detail).toContain(`Command line: "${INSTALL}\\programs`);
    // Commas inside a quoted field stay in the field.
    expect(rows[1]?.detail).toContain('Disposition: OpenIf, Options:');
    expect(
      parseProcmonCsv(
        '"Time of Day","Process Name","PID","Operation","Path","Result","Detail"\r\n"1","a.exe","2","WriteFile","C:\\x","SUCCESS","multi\r\nline"\r\n',
      )[0]?.detail,
    ).toBe('multi\r\nline');
    expect(() => parseProcmonCsv('"Time of Day","PID"\n"1","2"\n')).toThrow(/Process Name/);
    expect(() => parseProcmonCsv('')).toThrow(/empty/);
  });

  test('only_successful_writes_count_as_writes', () => {
    const create = (detail: string, result = 'SUCCESS'): ProcmonRow =>
      row({ operation: 'CreateFile', detail, result });
    expect(isWrite(row({ operation: 'WriteFile' }))).toBe(true);
    expect(isWrite(row({ operation: 'WriteFile', result: 'ACCESS DENIED' }))).toBe(false);
    expect(isWrite(row({ operation: 'ReadFile' }))).toBe(false);
    expect(isWrite(row({ operation: 'QueryOpen' }))).toBe(false);
    expect(isWrite(create('Desired Access: Generic Write, OpenResult: Created'))).toBe(true);
    expect(isWrite(create('Desired Access: Read Attributes, OpenResult: Created'))).toBe(true);
    expect(isWrite(create('Desired Access: Generic Read/Write, OpenResult: Opened'))).toBe(true);
    expect(isWrite(create('Desired Access: Delete, Synchronize, OpenResult: Opened'))).toBe(true);
    expect(
      isWrite(create('Desired Access: Read Attributes, Synchronize, OpenResult: Opened')),
    ).toBe(false);
    expect(
      isWrite(create('Desired Access: Generic Write, OpenResult: Created', 'NAME NOT FOUND')),
    ).toBe(false);
    expect(isWrite(row({ operation: 'RegSetValue', path: 'HKCU\\Software\\X' }))).toBe(true);
    expect(isWrite(row({ operation: 'RegQueryValue', path: 'HKCU\\Software\\X' }))).toBe(false);
    expect(isWrite(row({ operation: 'RegCreateKey', detail: 'Desired Access: All Access' }))).toBe(
      false,
    );
    expect(
      isWrite(row({ operation: 'RegCreateKey', detail: 'Disposition: REG_CREATED_NEW_KEY' })),
    ).toBe(true);
    // Only the access flags count, up to Disposition: a later "Write Through" option is not a write.
    expect(
      isWrite(
        create(
          'Desired Access: Read Attributes, Generic Write, Disposition: Open, Options: Non-Directory File',
        ),
      ),
    ).toBe(true);
    expect(
      isWrite(create('Desired Access: Read Attributes, Disposition: Open, Options: Write Through')),
    ).toBe(false);
    expect(isWrite(row({ operation: 'SetRenameInformationFile' }))).toBe(true);
    expect(isWrite(row({ operation: 'SetDispositionInformationFile' }))).toBe(true);
  });

  test('the_launcher_process_tree_follows_process_create_and_ignores_other_programs', async () => {
    const rows = parseProcmonCsv(await fixtureText());
    expect([...launcherProcessIds(rows, 'slatecore-launcher.exe')].sort()).toEqual([
      2000, 3000, 3100,
    ]);
    expect(launcherProcessIds(rows, 'SLATECORE-LAUNCHER.EXE').has(2000)).toBe(true);
    expect(launcherProcessIds(rows, 'nothing.exe').size).toBe(0);
  });

  test('allowlist_semantics_inside_install_dir_is_fine_and_anything_else_needs_a_rule', async () => {
    const rows = parseProcmonCsv(await fixtureText());
    const report = checkTrace(rows, {
      installDir: INSTALL,
      allowlist: await shippedAllowlist(),
      env: ENV,
    });
    expect(report.violations.map((v) => [v.process, v.operation, v.path])).toEqual([
      ['slatecore-launcher.exe', 'RegSetValue', 'HKCU\\Software\\Microsoft\\Edge\\BLBeacon\\state'],
      [
        'msedgewebview2.exe',
        'WriteFile',
        `${ENV.LOCALAPPDATA}\\xyz.genslate.slatecore.launcher\\EBWebView\\Default\\Preferences`,
      ],
      ['slatecore-launcher.exe', 'CreateFile', `${ENV.LOCALAPPDATA}\\Temp\\scratch.tmp`],
    ]);
    // Writes inside installDir (4) and the documented GPU shader cache (1) are explained.
    expect(report.allowed).toEqual({ 'install-dir': 4, 'gpu-shader-cache-d3d': 1 });
    expect(report.writes).toBe(8);
    // Unrelated programs (chrome.exe, a WebView2 app the launcher did not start) are not judged,
    // and neither are failed attempts or reads.
    expect(report.violations.some((v) => v.process === 'chrome.exe' || v.pid === 5000)).toBe(false);
  });

  test('a_rule_matches_the_whole_path_without_regard_to_letter_case', () => {
    expect(globToRegExp('E:\\Test Install\\**').test('e:\\test install\\Other\\a.txt')).toBe(true);
    // Not a prefix trick: a sibling folder with the same start is outside.
    expect(globToRegExp('E:\\Test Install\\**').test('E:\\Test Install2\\a.txt')).toBe(false);
    expect(globToRegExp('C:\\a\\*').test('C:\\a\\b.txt')).toBe(true);
    expect(globToRegExp('C:\\a\\*').test('C:\\a\\b\\c.txt')).toBe(false);
    expect(globToRegExp('C:\\a\\**').test('C:\\a\\b\\c.txt')).toBe(true);
    expect(globToRegExp('C:\\a.b\\**').test('C:\\aXb\\c.txt')).toBe(false);
  });

  test('install_dir_with_a_trailing_slash_and_other_case_still_matches', async () => {
    const report = checkTrace([row({ pid: 1, path: 'e:\\TEST INSTALL\\other\\x' })], {
      installDir: `${INSTALL}\\`,
      allowlist: await shippedAllowlist(),
      env: ENV,
    });
    expect(report.violations).toEqual([]);
    expect(report.allowed).toEqual({ 'install-dir': 1 });
  });

  test('the_shipped_allowlist_is_valid_and_every_rule_says_why', async () => {
    const list = await shippedAllowlist();
    expect(list.root).toBe('slatecore-launcher.exe');
    expect(list.rules.map((rule) => rule.id)).toContain('install-dir');
    for (const rule of list.rules) {
      expect(rule.reason.length, rule.id).toBeGreaterThan(30);
      // No rule may allow a whole drive or a whole user profile.
      expect(rule.path, rule.id).not.toMatch(/^[A-Za-z]:\\\*\*$|^%[A-Za-z]+%\\\*\*$/);
    }
    expect(() => parseAllowlist({ schema: 2, root: 'x', rules: [] })).toThrow(/schema/);
    expect(() =>
      parseAllowlist({
        schema: 1,
        root: 'x',
        rules: [{ id: 'a', reason: 'too short', path: '{installDir}\\**' }],
      }),
    ).toThrow(/reason/);
    expect(() =>
      parseAllowlist({
        schema: 1,
        root: 'x',
        rules: [{ id: 'a', reason: 'a good long reason', path: 'C:\\Users\\**' }],
      }),
    ).toThrow(/must start with/);
    const twin = { id: 'a', reason: 'a good long reason', path: '{installDir}\\**' };
    expect(() => parseAllowlist({ schema: 1, root: 'x', rules: [twin, twin] })).toThrow(
      /duplicate/,
    );
  });

  test('procmon_cli_fails_on_violations_passes_a_clean_trace_and_never_needs_procmon', async () => {
    const dir = await tempDir();
    const dirty = join(dir, 'dirty.csv');
    await writeFile(dirty, await fixtureText());
    const clean = join(dir, 'clean.csv');
    const keep = (await fixtureText())
      .split('\n')
      .filter((line) => !/BLBeacon|Preferences|scratch\.tmp/.test(line))
      .join('\n');
    await writeFile(clean, keep);
    const empty = join(dir, 'empty.csv');
    await writeFile(
      empty,
      '"Time of Day","Process Name","PID","Operation","Path","Result","Detail"\n"1","chrome.exe","9","WriteFile","C:\\x","SUCCESS",""\n',
    );

    async function cli(...args: string[]): Promise<{ code: number; out: string }> {
      const child = Bun.spawn(['bun', join(ROOT, 'scripts/no-trace/procmon.ts'), ...args], {
        stdout: 'pipe',
        stderr: 'pipe',
        env: { ...process.env, ...ENV, PROCMON_PATH: '' },
      });
      const [out, err, code] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
        child.exited,
      ]);
      return { code, out: out + err };
    }
    const failing = await cli('--csv', dirty, '--install-dir', INSTALL);
    expect(failing.code).toBe(1);
    expect(failing.out).toContain('VIOLATION');
    expect(failing.out).toContain('BLBeacon');
    expect((await cli('--csv', clean, '--install-dir', INSTALL)).code).toBe(0);
    // A trace without the launcher in it checked nothing: that is a failure, not a pass.
    const nothing = await cli('--csv', empty, '--install-dir', INSTALL);
    expect(nothing.code).toBe(1);
    expect(nothing.out).toContain('nothing was checked');
    expect((await cli('--install-dir', INSTALL)).code).toBe(2);
    expect((await cli('--pml', join(dir, 'x.pml'), '--install-dir', INSTALL)).code).toBe(2);
  });

  test('pml_conversion_uses_the_process_monitor_command_line', () => {
    expect(pmlToCsvArgs('Procmon64.exe', 'a.pml', 'a.csv')).toEqual([
      'Procmon64.exe',
      '/AcceptEula',
      '/Quiet',
      '/OpenLog',
      'a.pml',
      '/SaveAs',
      'a.csv',
    ]);
  });
});

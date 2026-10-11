/**
 * Tier 2 of the portability test (`scripts/tests/no-trace.test.ts`): run the packaged exe on
 * Windows and prove it left nothing on the PC. The helpers here are plain functions so the parts
 * that need no Windows (listing, diffing, parsing a `reg export`) are tested everywhere.
 *
 * Method: snapshot `%TEMP%`, `%APPDATA%`, `%LOCALAPPDATA%` and `HKCU\Software`, start the
 * launcher, wait until it is ready, end it, snapshot again. A control run with no launcher (the
 * same snapshots around the same wait) measures the PC's own background noise; what the launcher
 * run changed beyond the control run is a leak.
 */
import { createHash } from 'node:crypto';
import { copyFile, mkdir, readdir, readFile, rm, stat } from 'node:fs/promises';
import { join } from 'node:path';

import { readZip } from '../lib/archive';
import { EXE_ZIP_PATH, systemExe } from '../lib/targets';
import { globToRegExp } from './procmon';

/**
 * A folder listing (`null`: the folder does not exist): entry name to a signature. Relative
 * names, folders end in `/` (signature empty), files carry `size:mtime` so an edit to an
 * existing file shows up as a change.
 */
export type Listing = Readonly<Record<string, string>> | null;

/** What Tier 2 compares: the three OS folders and the registry hive. */
export interface HostSnapshot {
  readonly folders: Readonly<Record<string, Listing>>;
  /** Registry key path to a hash of its values. */
  readonly registry: Readonly<Record<string, string>>;
}

/** The folders to watch, by label. A missing variable is simply not watched. */
export type HostPlaces = Readonly<Record<string, string | undefined>>;

/** Lists `dir` down to `depth` levels (1 is its own entries). */
export async function listFolder(dir: string, depth: number): Promise<Listing> {
  const found: Record<string, string> = {};
  async function visit(relative: string, level: number): Promise<void> {
    const entries = await readdir(join(dir, relative), { withFileTypes: true }).catch(
      () => undefined,
    );
    if (entries === undefined) return;
    for (const entry of entries) {
      const name = relative === '' ? entry.name : `${relative}/${entry.name}`;
      if (entry.isDirectory()) {
        found[`${name}/`] = '';
        if (level < depth) await visit(name, level + 1);
      } else {
        const info = await stat(join(dir, name)).catch(() => undefined);
        found[name] = info === undefined ? '?' : `${info.size}:${Math.trunc(info.mtimeMs)}`;
      }
    }
  }
  const top = await readdir(dir).catch(() => undefined);
  if (top === undefined) return null;
  await visit('', 1);
  return Object.fromEntries(Object.entries(found).sort(([a], [b]) => (a < b ? -1 : 1)));
}

/**
 * Parses a `reg export` into key path to a hash of the key's lines. Values are hashed, not kept,
 * so a changed value is seen without keeping a copy of the user's registry in memory or on disk.
 */
export function parseRegExport(text: string): Record<string, string> {
  const keys: Record<string, string> = {};
  let key: string | undefined;
  let lines: string[] = [];
  const close = (): void => {
    if (key !== undefined) keys[key] = createHash('sha1').update(lines.join('\n')).digest('hex');
  };
  for (const line of text.replace(/^\uFEFF/, '').split(/\r?\n/)) {
    const header = /^\[(.+)\]$/.exec(line);
    if (header?.[1] !== undefined) {
      close();
      key = header[1];
      lines = [];
    } else if (key !== undefined && line !== '') {
      lines.push(line);
    }
  }
  close();
  return keys;
}

/**
 * What changed between two snapshots: `+ added`, `- removed` and `~ changed` lines, sorted. A
 * file whose size or modification time changed is `~`; a key whose values changed is `~`.
 */
export function diffSnapshots(before: HostSnapshot, after: HostSnapshot): string[] {
  const changes: string[] = [];
  for (const label of new Set([...Object.keys(before.folders), ...Object.keys(after.folders)])) {
    const was = before.folders[label] ?? null;
    const now = after.folders[label] ?? null;
    if (was === null && now !== null) changes.push(`+ ${label} (the folder itself)`);
    if (was !== null && now === null) changes.push(`- ${label} (the folder itself)`);
    for (const name of new Set([...Object.keys(was ?? {}), ...Object.keys(now ?? {})])) {
      const before = was?.[name];
      const after = now?.[name];
      if (before === undefined) changes.push(`+ ${label}/${name}`);
      else if (after === undefined) changes.push(`- ${label}/${name}`);
      else if (before !== after) changes.push(`~ ${label}/${name}`);
    }
  }
  for (const key of new Set([...Object.keys(before.registry), ...Object.keys(after.registry)])) {
    const was = before.registry[key];
    const now = after.registry[key];
    if (was === undefined) changes.push(`+ ${key}`);
    else if (now === undefined) changes.push(`- ${key}`);
    else if (was !== now) changes.push(`~ ${key}`);
  }
  return changes.sort();
}

/** The changes of the launcher run that the control run did not also show. */
export function leaksOf(control: readonly string[], app: readonly string[]): string[] {
  const noise = new Set(control);
  return app.filter((change) => !noise.has(change));
}

/** One thing Windows itself records when any new program runs or a tray icon appears. */
export interface IgnoreRule {
  readonly id: string;
  readonly reason: string;
  /** A glob over a registry key (`HKEY_CURRENT_USER\...`) or a folder entry (`LABEL/...`). The
   * key or entry and everything below it match. Case-insensitive. */
  readonly path: string;
}

export interface IgnoreList {
  readonly schema: 1;
  /** The list is a judgement from knowledge, not from a trace: true until a real Windows run. */
  readonly provisional: boolean;
  readonly rules: readonly IgnoreRule[];
}

/** Validates the parsed `tier2-ignore.json`. */
export function parseIgnoreList(value: unknown): IgnoreList {
  const raw = value as { schema?: unknown; provisional?: unknown; rules?: unknown };
  if (raw.schema !== 1) throw new Error('tier 2 ignore list: schema must be 1');
  if (typeof raw.provisional !== 'boolean') {
    throw new Error('tier 2 ignore list: "provisional" must say whether a real run confirmed it');
  }
  if (!Array.isArray(raw.rules)) throw new Error('tier 2 ignore list: rules must be a list');
  const seen = new Set<string>();
  const rules = (raw.rules as Record<string, unknown>[]).map((rule) => {
    const { id, reason, path } = rule;
    if (typeof id !== 'string' || typeof reason !== 'string' || typeof path !== 'string') {
      throw new Error('tier 2 ignore list: every rule needs a string id, reason and path');
    }
    if (seen.has(id)) throw new Error(`tier 2 ignore list: duplicate rule id "${id}"`);
    seen.add(id);
    if (reason.trim().length < 20) throw new Error(`tier 2 ignore list: "${id}" needs a reason`);
    if (!/^(HKEY_CURRENT_USER\\|(TEMP|APPDATA|LOCALAPPDATA)\/)/.test(path)) {
      throw new Error(
        `tier 2 ignore list: "${id}" must start inside HKEY_CURRENT_USER or a watched folder`,
      );
    }
    const parts = path.split(/[\\/]/);
    const registry = path.startsWith('HKEY_');
    if (parts.length < (registry ? 4 : 2) || parts.slice(1, 3).some((part) => part.includes('*'))) {
      throw new Error(`tier 2 ignore list: "${id}" is too broad`);
    }
    return { id, reason, path };
  });
  return { schema: 1, provisional: raw.provisional, rules };
}

/** Drops the changes an ignore rule explains. A change line is `+ x`, `- x` or `~ x`. */
export function filterIgnored(changes: readonly string[], rules: readonly IgnoreRule[]): string[] {
  const matchers = rules.map((rule) => {
    const separator = rule.path.startsWith('HKEY_') ? '\\' : '/';
    return [globToRegExp(rule.path), globToRegExp(`${rule.path}${separator}**`)] as const;
  });
  return changes.filter((change) => {
    const target = change.slice(2).replace(/ \(the folder itself\)$/, '');
    return !matchers.some((pair) => pair.some((pattern) => pattern.test(target)));
  });
}

/** Snapshots `places` and the registry text returned by `exportRegistry`. */
export async function takeSnapshot(
  places: HostPlaces,
  exportRegistry: () => Promise<string>,
  depth = 2,
): Promise<HostSnapshot> {
  const folders: Record<string, Listing> = {};
  for (const [label, dir] of Object.entries(places)) {
    if (dir !== undefined && dir !== '') folders[label] = await listFolder(dir, depth);
  }
  return { folders, registry: parseRegExport(await exportRegistry()) };
}

/** `reg export HKCU\Software` through Windows' own `reg.exe` (an argv, never a shell string). */
export async function exportHkcuSoftware(workDir: string): Promise<string> {
  await mkdir(workDir, { recursive: true });
  const file = join(workDir, 'hkcu-software.reg');
  await rm(file, { force: true });
  try {
    const child = Bun.spawn([systemExe('reg.exe'), 'export', 'HKCU\\Software', file, '/y'], {
      stdout: 'ignore',
      stderr: 'pipe',
      env: { ...process.env, MSYS_NO_PATHCONV: '1' },
    });
    const [stderr, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
    if (code !== 0) throw new Error(`reg export failed (${code}): ${stderr.trim()}`);
    // `reg export` writes UTF-16 with a byte order mark.
    return new TextDecoder('utf-16le').decode(await readFile(file));
  } finally {
    // The export holds the whole hive: it is deleted as soon as it was read.
    await rm(file, { force: true });
  }
}

/** Facts that decide whether Tier 2 can run here. */
export interface Tier2Facts {
  readonly platform: string;
  /** `undefined` when not Windows or not known. */
  readonly elevated: boolean | undefined;
  readonly zipPath: string | undefined;
  /** `true` when the exe in the zip starts with `MZ` and is more than a few bytes. */
  readonly exeIsReal: boolean | undefined;
  /** `SLATECORE_NO_TRACE=0`. */
  readonly disabled: boolean;
}

/** Why Tier 2 cannot run (shown in the test name and on the console), or `undefined` to run. */
export function tier2SkipReason(facts: Tier2Facts): string | undefined {
  if (facts.disabled) return 'switched off with SLATECORE_NO_TRACE=0';
  if (facts.platform !== 'win32') {
    return 'needs Windows (the packaged exe, WebView2 and the registry); run it on the Windows machine';
  }
  if (facts.elevated === true) {
    return 'this shell is elevated and the launcher refuses to run as administrator; use a normal prompt';
  }
  if (facts.zipPath === undefined) {
    return 'no release zip; run `bun run package` first (or set SLATECORE_NO_TRACE_ZIP)';
  }
  if (facts.exeIsReal === false) {
    return 'the zip holds a placeholder exe, not a built launcher; run `bun run package`';
  }
  return undefined;
}

/** Reads the facts of this machine. Cheap; spawns `fltmc` on Windows only. */
export async function gatherFacts(
  zipPath: string | undefined,
  env: Readonly<Record<string, string | undefined>> = process.env,
): Promise<Tier2Facts> {
  const platform = process.platform;
  let exeIsReal: boolean | undefined;
  if (zipPath !== undefined) {
    const exe = ((await readZip(zipPath).catch(() => ({}))) as Record<string, Uint8Array>)[
      EXE_ZIP_PATH
    ];
    exeIsReal = exe !== undefined && exe.length > 64 * 1024 && exe[0] === 0x4d && exe[1] === 0x5a;
  }
  // `fltmc` needs administrator rights: it succeeds only in an elevated shell.
  const elevated =
    platform === 'win32'
      ? Bun.spawnSync([systemExe('fltmc.exe')], { stdout: 'ignore', stderr: 'ignore' }).exitCode ===
        0
      : undefined;
  return { platform, elevated, zipPath, exeIsReal, disabled: env['SLATECORE_NO_TRACE'] === '0' };
}

export interface RunResult {
  /** The launcher logged that it started in suite mode. */
  readonly ready: boolean;
  /** The last lines of its log, for the failure message. */
  readonly logTail: string;
}

const sleep = (ms: number): Promise<void> => new Promise((done) => setTimeout(done, ms));

/** The launcher's environment: the caller's, without any `WEBVIEW2_*` of a developer PC. */
export function launcherEnv(
  env: Readonly<Record<string, string | undefined>>,
): Record<string, string> {
  const clean: Record<string, string> = { MSYS_NO_PATHCONV: '1' };
  for (const [name, value] of Object.entries(env)) {
    if (value !== undefined && !name.toUpperCase().startsWith('WEBVIEW2_')) clean[name] = value;
  }
  return clean;
}

/**
 * Starts `exe` (inside an unpacked install folder), waits until its log says it started, lets it
 * settle, then ends the whole process tree. There is no automation to open Settings or use the
 * vault, and no way to ask a hidden tray app to quit: it is ended with `taskkill /T /F`.
 */
export async function runLauncher(options: {
  readonly exe: string;
  readonly installDir: string;
  readonly readyTimeoutMs: number;
  readonly settleMs: number;
}): Promise<RunResult> {
  const { exe, installDir } = options;
  const logFile = join(installDir, 'other', 'launcher', 'logs', 'launcher.log');
  const child = Bun.spawn([exe], {
    cwd: join(exe, '..'),
    env: launcherEnv(process.env),
    stdin: 'ignore',
    stdout: 'ignore',
    stderr: 'ignore',
  });
  let exited = false;
  void child.exited.then(() => {
    exited = true;
  });
  let ready = false;
  let log = '';
  try {
    const deadline = Date.now() + options.readyTimeoutMs;
    while (Date.now() < deadline && !exited) {
      log = await readFile(logFile, 'utf8').catch(() => '');
      if (/\(Suite mode\)/.test(log)) {
        ready = true;
        break;
      }
      await sleep(500);
    }
    if (ready) await sleep(options.settleMs);
    log = await readFile(logFile, 'utf8').catch(() => log);
  } finally {
    const kill = Bun.spawn([systemExe('taskkill.exe'), '/PID', String(child.pid), '/T', '/F'], {
      stdout: 'ignore',
      stderr: 'ignore',
      env: { ...process.env, MSYS_NO_PATHCONV: '1' },
    });
    await kill.exited;
    await child.exited;
    // WebView2 children and Windows take a moment to let go of their files.
    await sleep(2_000);
  }
  return { ready, logTail: log.split(/\r?\n/).slice(-15).join('\n') };
}

/**
 * The control run: starts a harmless program that is not the launcher (a copy of `cmd.exe`, run
 * with `/c exit`, so it does nothing) and waits `durationMs`. Windows writes some of its own
 * bookkeeping for every new exe it sees (the Program Compatibility Assistant store, MuiCache,
 * UserAssist); the copy attracts the same, so that noise appears in both runs and cancels.
 */
export async function runControl(options: {
  readonly workDir: string;
  readonly durationMs: number;
}): Promise<void> {
  await mkdir(options.workDir, { recursive: true });
  const exe = join(options.workDir, 'slatecore-control.exe');
  await copyFile(systemExe('cmd.exe'), exe);
  const child = Bun.spawn([exe, '/c', 'exit'], {
    cwd: options.workDir,
    env: launcherEnv(process.env),
    stdin: 'ignore',
    stdout: 'ignore',
    stderr: 'ignore',
  });
  await child.exited;
  await sleep(options.durationMs);
}

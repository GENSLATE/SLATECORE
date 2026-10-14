/**
 * The release step behind `bun run package`: the version, the pinned WebView2 runtime
 * (`scripts/webview2.lock.json`), the add-on zip, and `release/` housekeeping (older builds move
 * to `release/.archive/`). Every check that can refuse runs before the first byte is written.
 */
import { mkdir, mkdtemp, open, readdir, readFile, rename, rm, stat } from 'node:fs/promises';
import { join } from 'node:path';

import { sha256File, writeZip, type ZipEntry } from './archive';
import { planBase, planRuntime, type RuntimeFacts } from './stage';
import {
  ARCHIVE_DIR,
  BUILDING_DIR,
  baseZipName,
  GENSLATE_LICENSE,
  LAUNCHER_DIR,
  LOCK_FILE,
  MICROSOFT_DOMAIN,
  RELEASE_PREFIX,
  RUNTIME_EXE,
  RUNTIME_FOLDER,
  runtimeZipName,
  SEED_APP_DIR,
  systemExe,
} from './targets';

const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const UNRESOLVED = 'UNRESOLVED';

/**
 * The version of the repo: the root `package.json`, which `bun run version` keeps in step. The
 * launcher's own `package.json` must say the same, because that is the version the exe embeds.
 */
export async function readVersion(root: string): Promise<string> {
  const read = async (file: string): Promise<unknown> =>
    (JSON.parse(await readFile(join(root, file), 'utf8')) as { version?: unknown }).version;
  const version = await read('package.json');
  if (typeof version !== 'string' || !SEMVER.test(version)) {
    throw new Error(`the root package.json version "${String(version)}" is not semver`);
  }
  const embedded = await read(`${LAUNCHER_DIR}/package.json`);
  if (embedded !== version) {
    throw new Error(
      `the launcher's package.json version "${String(embedded)}" is not the root version "${version}", so the exe would report another version than the zip name; run \`bun run version\``,
    );
  }
  return version;
}

/** `true` when the file starts with the `MZ` of a Windows program. */
export async function looksLikeWindowsExe(path: string): Promise<boolean> {
  const handle = await open(path, 'r');
  try {
    const head = new Uint8Array(2);
    const { bytesRead } = await handle.read(head, 0, 2, 0);
    return bytesRead === 2 && head[0] === 0x4d && head[1] === 0x5a;
  } finally {
    await handle.close();
  }
}

/**
 * A tag build publishes `slatecore-launcher-<version>-*.zip`, so the pushed tag has to be
 * `v<version>`: a `v1.0.0` tag on a 0.1.0 package would ship 0.1.0 zips under a 1.0.0 release.
 * Other builds (branches, pull requests, a local run) are not tagged and pass.
 */
export function assertTagMatchesVersion(
  env: Readonly<Record<string, string | undefined>>,
  version: string,
): void {
  if (env['GITHUB_REF_TYPE'] !== 'tag') return;
  const name = env['GITHUB_REF_NAME'];
  if (name === undefined || name === '') {
    throw new Error('a tag build needs GITHUB_REF_NAME to check the tag against the version');
  }
  if (name !== `v${version}`) {
    throw new Error(
      `the tag ${name} does not match the launcher version: expected v${version} (the zips are named for ${version})`,
    );
  }
}

/**
 * The license files a release must carry but that are empty or missing: the GENSLATE license
 * text is the owner's to supply, and nobody else writes it.
 */
export async function licenseGaps(root: string): Promise<string[]> {
  const seed = join(root, SEED_APP_DIR, 'licenses', 'genslate.LICENSE.md');
  const info = await stat(seed).catch(() => undefined);
  return info?.isFile() === true && info.size > 0 ? [] : [GENSLATE_LICENSE];
}

/** `scripts/webview2.lock.json`: which Microsoft file the runtime add-on is made from. */
export interface Webview2Lock {
  readonly schema: number;
  /** Runtime version, `154.0.4258.62`. */
  readonly version: string;
  readonly architecture: string;
  /** File name of the Fixed Version cab. */
  readonly fileName: string;
  /** Where Microsoft publishes it (https, a microsoft.com host). */
  readonly url: string;
  /** Lowercase hex SHA-256 of the cab. */
  readonly sha256: string;
  /** Size of the cab in bytes. */
  readonly size: number;
  /** Repo path of the licence notice that goes into the add-on. */
  readonly licenseNotice: string;
}

/** Reads a lock file's text; the values are checked by `assertLockResolved`, not here. */
export function parseLock(text: string): Webview2Lock {
  const raw = JSON.parse(text) as Record<string, unknown>;
  const str = (key: string): string => {
    const value = raw[key];
    if (typeof value !== 'string') throw new Error(`${LOCK_FILE}: "${key}" must be a string`);
    return value;
  };
  const size = raw['size'];
  const schema = raw['schema'];
  if (typeof size !== 'number') throw new Error(`${LOCK_FILE}: "size" must be a number`);
  if (typeof schema !== 'number') throw new Error(`${LOCK_FILE}: "schema" must be a number`);
  return {
    schema,
    version: str('version'),
    architecture: str('architecture'),
    fileName: str('fileName'),
    url: str('url'),
    sha256: str('sha256'),
    size,
    licenseNotice: str('licenseNotice'),
  };
}

function microsoftUrlProblem(value: string): string | undefined {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return `url "${value}" is not a URL`;
  }
  const host = url.hostname.toLowerCase();
  const microsoft = host === MICROSOFT_DOMAIN || host.endsWith(`.${MICROSOFT_DOMAIN}`);
  if (url.protocol !== 'https:' || !microsoft || url.username !== '' || url.password !== '') {
    return `url must be an https ${MICROSOFT_DOMAIN} address without credentials (the runtime is only taken from Microsoft), not "${value}"`;
  }
  return undefined;
}

/**
 * Throws unless the lock names a real, verified file: a version, a file name, a Microsoft
 * address, a 64 digit lowercase SHA-256 and a size. The lock ships with `UNRESOLVED` markers so
 * the add-on cannot be built until a person has downloaded the cab from Microsoft and filled
 * the values in (steps in `programs/desktop/launcher/other/launcher/documents/release.md`).
 */
export function assertLockResolved(lock: Webview2Lock): void {
  const problems: string[] = [];
  const marked = (value: string): boolean => value === UNRESOLVED || value === '';
  if (lock.schema !== 1) problems.push(`schema must be 1, not ${lock.schema}`);
  if (!/^\d+\.\d+\.\d+\.\d+$/.test(lock.version)) {
    problems.push(`version "${lock.version}" is not a runtime version like 154.0.4258.62`);
  }
  if (lock.architecture !== 'x64') problems.push(`architecture must be "x64"`);
  if (marked(lock.fileName) || !/^[\w.-]+\.cab$/.test(lock.fileName)) {
    problems.push(`fileName "${lock.fileName}" is not the name of a .cab file`);
  }
  const urlProblem = microsoftUrlProblem(lock.url);
  if (urlProblem !== undefined) problems.push(urlProblem);
  if (!/^[0-9a-f]{64}$/.test(lock.sha256) || /^0+$/.test(lock.sha256)) {
    problems.push(`sha256 "${lock.sha256}" is not a 64 digit lowercase SHA-256 of the cab`);
  }
  if (!Number.isInteger(lock.size) || lock.size <= 0) {
    problems.push(`size ${lock.size} must be the cab's size in bytes`);
  }
  if (lock.licenseNotice === '' || /^([a-z]:|\/)|\.\./i.test(lock.licenseNotice)) {
    problems.push('licenseNotice must be a repo-relative path');
  }
  if (problems.length > 0) {
    throw new Error(
      [
        `${LOCK_FILE} is unresolved or invalid, so the WebView2 runtime add-on will not be built:`,
        ...problems.map((problem) => `  ${problem}`),
        'Fill it from the cab downloaded from Microsoft (see documents/release.md).',
      ].join('\n'),
    );
  }
}

/** Moves every `slatecore-launcher-*.zip` of `releaseDir` into `.archive/`; returns the new paths. */
export async function archiveOldBuilds(releaseDir: string): Promise<string[]> {
  const names = await readdir(releaseDir).catch(() => [] as string[]);
  const old = names.filter((name) => name.startsWith(RELEASE_PREFIX) && name.endsWith('.zip'));
  if (old.length === 0) return [];
  const archive = join(releaseDir, ARCHIVE_DIR);
  await mkdir(archive, { recursive: true });
  const moved: string[] = [];
  for (const name of old.sort()) {
    const stem = name.slice(0, -'.zip'.length);
    let target = join(archive, name);
    for (let n = 2; await stat(target).catch(() => undefined); n++) {
      target = join(archive, `${stem}-${n}.zip`);
    }
    await rename(join(releaseDir, name), target);
    moved.push(target);
  }
  return moved;
}

/** Runs Windows' own `expand.exe` (not GNU `expand` from Git Bash) on the cab. */
export async function expandCab(cab: string, destination: string): Promise<void> {
  if (process.platform !== 'win32') throw new Error('expanding the WebView2 cab needs Windows');
  const expand = systemExe('expand.exe');
  await mkdir(destination, { recursive: true });
  const child = Bun.spawn([expand, cab, '-F:*', destination], {
    stdout: 'inherit',
    stderr: 'inherit',
  });
  const code = await child.exited;
  if (code !== 0) throw new Error(`expand.exe exited with code ${code}`);
}

export interface Webview2Options {
  /** The cab downloaded from Microsoft. */
  readonly cab: string;
  readonly lock: Webview2Lock;
  /** Replaces `expand.exe` (tests). */
  readonly extract?: (cab: string, destination: string) => Promise<void>;
}

export interface ReleaseOptions {
  readonly root: string;
  readonly releaseDir: string;
  readonly exe: string;
  /** Default: the repo version. */
  readonly version?: string;
  readonly webview2?: Webview2Options;
  /** A release build: a missing license text is an error, not a warning. */
  readonly release?: boolean;
  /** Scratch space for the extracted cab. Default: `<root>/target/package`. */
  readonly workDir?: string;
}

export interface ReleaseResult {
  readonly version: string;
  readonly base: string;
  readonly runtime?: string;
  /** Where the older builds went. */
  readonly archived: readonly string[];
  /** Problems that do not stop an ordinary build (a release build refuses them). */
  readonly warnings: readonly string[];
}

/** Refuses a cab that is not the file the lock names. */
async function verifyCab(webview2: Webview2Options): Promise<void> {
  assertLockResolved(webview2.lock);
  const info = await stat(webview2.cab).catch(() => undefined);
  if (info === undefined || !info.isFile()) throw new Error(`the cab is missing: ${webview2.cab}`);
  if (info.size !== webview2.lock.size) {
    throw new Error(
      `the cab's size ${info.size} is not the locked size ${webview2.lock.size} (${LOCK_FILE})`,
    );
  }
  const sha256 = await sha256File(webview2.cab);
  if (sha256 !== webview2.lock.sha256) {
    throw new Error(
      `the cab's sha256 ${sha256} is not the locked sha256 ${webview2.lock.sha256} (${LOCK_FILE}); it may not be the file from Microsoft`,
    );
  }
}

/** Finds the extracted runtime folder: the Microsoft-named one, else `extracted` itself. */
async function findRuntimeFolder(extracted: string, version: string): Promise<string> {
  const names = await readdir(extracted);
  const named = names.find((name) => RUNTIME_FOLDER.test(name));
  const folder = named === undefined ? extracted : join(extracted, named);
  const found = RUNTIME_FOLDER.exec(named ?? '')?.[1];
  if (found !== undefined && found !== version) {
    throw new Error(`the cab holds runtime ${found}, but the lock pins ${version}`);
  }
  if (!(await stat(join(folder, RUNTIME_EXE)).catch(() => undefined))?.isFile()) {
    throw new Error(`the extracted runtime has no ${RUNTIME_EXE}`);
  }
  return folder;
}

/**
 * Packages the base zip (and, with `webview2`, the runtime add-on) into `releaseDir`. Older
 * builds are archived only after the new zips exist, so a failed run changes nothing.
 */
export async function buildRelease(options: ReleaseOptions): Promise<ReleaseResult> {
  const { root, releaseDir, exe, webview2 } = options;
  const version = options.version ?? (await readVersion(root));

  const gaps = await licenseGaps(root);
  const warnings = gaps.map(
    (path) =>
      `${path} is empty: the GENSLATE license text has to come from the owner before a tagged release`,
  );
  if (options.release === true && warnings.length > 0) {
    throw new Error(`refusing a release build: ${warnings.join('; ')}`);
  }
  if (webview2 !== undefined) await verifyCab(webview2);
  const baseEntries = await planBase({ root, exe, version });

  const work = options.workDir ?? join(root, 'target', 'package');
  let extracted: string | undefined;
  const building = join(releaseDir, BUILDING_DIR);
  // Once the new zips exist they are kept if anything later fails: the build is never lost.
  let keepBuilding = false;
  try {
    let runtimeEntries: ZipEntry[] | undefined;
    if (webview2 !== undefined) {
      await mkdir(work, { recursive: true });
      extracted = await mkdtemp(join(work, 'webview2-'));
      await (webview2.extract ?? expandCab)(webview2.cab, extracted);
      const runtimeDir = await findRuntimeFolder(extracted, webview2.lock.version);
      const facts: RuntimeFacts = {
        version: webview2.lock.version,
        sha256: webview2.lock.sha256,
        source: webview2.lock.url,
        fileName: webview2.lock.fileName,
      };
      const notice = await readFile(join(root, webview2.lock.licenseNotice), 'utf8');
      runtimeEntries = await planRuntime({ runtimeDir, facts, notice });
    }

    await rm(building, { recursive: true, force: true });
    await mkdir(building, { recursive: true });
    await writeZip(baseEntries, join(building, baseZipName(version)));
    keepBuilding = true;
    try {
      if (runtimeEntries !== undefined) {
        await writeZip(runtimeEntries, join(building, runtimeZipName(version)));
      }
      const archived = await archiveOldBuilds(releaseDir);
      const base = join(releaseDir, baseZipName(version));
      await rename(join(building, baseZipName(version)), base);
      let runtime: string | undefined;
      if (runtimeEntries !== undefined) {
        runtime = join(releaseDir, runtimeZipName(version));
        await rename(join(building, runtimeZipName(version)), runtime);
      }
      keepBuilding = false;
      return { version, base, ...(runtime === undefined ? {} : { runtime }), archived, warnings };
    } catch (error) {
      const kept = (await readdir(building).catch(() => [] as string[])).map((name) =>
        join(building, name),
      );
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`${reason}\nthe new build is kept: ${kept.join(', ')}`, { cause: error });
    }
  } finally {
    if (!keepBuilding) await rm(building, { recursive: true, force: true });
    if (extracted !== undefined) await rm(extracted, { recursive: true, force: true });
  }
}

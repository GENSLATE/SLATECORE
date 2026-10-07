import { join } from 'node:path';

const SEMVER = /^(\d+)\.(\d+)\.(\d+)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

/** One file whose text changes with the version. */
export interface VersionEdit {
  /** Path relative to the repo root, `/` separated. */
  readonly file: string;
  readonly before: string;
  readonly after: string;
}

/** `patch | minor | major | x.y.z` applied to `current`. */
export function nextVersion(current: string, bump: string): string {
  if (SEMVER.test(bump)) return bump;
  const match = SEMVER.exec(current);
  if (match === null) throw new Error(`current version "${current}" is not semver`);
  const [major, minor, patch] = [Number(match[1]), Number(match[2]), Number(match[3])];
  switch (bump) {
    case 'major':
      return `${major + 1}.0.0`;
    case 'minor':
      return `${major}.${minor + 1}.0`;
    case 'patch':
      return `${major}.${minor}.${patch + 1}`;
    default:
      throw new Error(`invalid bump "${bump}" (use patch, minor, major or x.y.z)`);
  }
}

/** Sets the first `"version": "..."` of a package.json text. */
export function setPackageVersion(text: string, version: string): string {
  return text.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`);
}

/**
 * Sets `version = "..."` inside the `[workspace.package]` table of a Cargo.toml text (line
 * based, so other arrays in the table cannot confuse it).
 */
export function setCargoWorkspaceVersion(text: string, version: string): string {
  const lines = text.split('\n');
  const start = lines.findIndex((line) => line.trim() === '[workspace.package]');
  if (start === -1) return text;
  for (let index = start + 1; index < lines.length; index++) {
    const line = lines[index] ?? '';
    if (/^\s*\[/.test(line)) break;
    if (/^\s*version\s*=/.test(line)) {
      lines[index] = line.replace(/(version\s*=\s*")[^"]*(")/, `$1${version}$2`);
      break;
    }
  }
  return lines.join('\n');
}

/**
 * Sets a literal `"version"` of a tauri.conf.json text. A path such as `"../package.json"` is
 * left alone because it already follows its package.json.
 */
export function setTauriConfVersion(text: string, version: string): string {
  return text.replace(/("version"\s*:\s*")\d[^"]*(")/, `$1${version}$2`);
}

async function glob(root: string, pattern: string): Promise<string[]> {
  const files: string[] = [];
  for await (const file of new Bun.Glob(pattern).scan({ cwd: root })) {
    files.push(file.replaceAll('\\', '/'));
  }
  return files;
}

/** What `bun run version <bump>` would change under `root`. */
export async function planVersionBump(
  root: string,
  bump: string,
): Promise<{ readonly current: string; readonly next: string; readonly edits: VersionEdit[] }> {
  const rootPackage = JSON.parse(await Bun.file(join(root, 'package.json')).text()) as {
    version?: string;
  };
  if (rootPackage.version === undefined) throw new Error('root package.json has no version');
  const current = rootPackage.version;
  const next = nextVersion(current, bump);

  const targets: { file: string; update: (text: string) => string }[] = [];
  for (const pattern of [
    'package.json',
    'programs/desktop/*/package.json',
    'programs/webapp/*/package.json',
    'packages/*/package.json',
  ]) {
    for (const file of await glob(root, pattern)) {
      targets.push({ file, update: (text) => setPackageVersion(text, next) });
    }
  }
  targets.push({ file: 'Cargo.toml', update: (text) => setCargoWorkspaceVersion(text, next) });
  for (const file of await glob(root, 'programs/desktop/*/src-tauri/tauri.conf.json')) {
    targets.push({ file, update: (text) => setTauriConfVersion(text, next) });
  }

  const edits: VersionEdit[] = [];
  for (const { file, update } of targets.sort((a, b) => a.file.localeCompare(b.file))) {
    const handle = Bun.file(join(root, file));
    if (!(await handle.exists())) continue;
    const before = await handle.text();
    const after = update(before);
    if (after !== before) edits.push({ file, before, after });
  }
  return { current, next, edits };
}

/** Writes the planned edits. */
export async function writeVersionEdits(
  root: string,
  edits: readonly VersionEdit[],
): Promise<void> {
  for (const edit of edits) await Bun.write(join(root, edit.file), edit.after);
}

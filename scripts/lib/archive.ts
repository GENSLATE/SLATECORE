/**
 * Zip files with `fflate`: a streaming writer (memory stays at one file, the WebView2 add-on is
 * hundreds of megabytes), a reader for tests, a safe extractor and a streaming SHA-256.
 *
 * Entry names are relative, `/` separated and never leave the folder they are extracted to.
 * Output is reproducible: entries are sorted and carry one fixed time stamp.
 */
import { closeSync, openSync, renameSync, rmSync, writeSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

import { unzipSync, Zip, ZipDeflate, ZipPassThrough } from 'fflate';

/** Where an entry's bytes come from. */
export type ZipSource =
  | { readonly kind: 'file'; readonly path: string }
  | { readonly kind: 'bytes'; readonly bytes: Uint8Array }
  | { readonly kind: 'dir' };

/** One thing to put in a zip; `path` ends with `/` for a folder. */
export interface ZipEntry {
  readonly path: string;
  readonly source: ZipSource;
}

/** The time stamp of every entry (local time, so the DOS time field is the same everywhere). */
const STAMP = new Date(2000, 0, 1, 0, 0, 0);
/** DOS "directory" attribute. */
const DIRECTORY_ATTRIBUTE = 0x10;

/**
 * Checks a zip entry name and returns it: relative, `/` separated, no `.` or `..` segment, no
 * empty segment, no drive colon. A trailing `/` marks a folder.
 */
export function safeEntryPath(name: string): string {
  const body = name.endsWith('/') ? name.slice(0, -1) : name;
  const bad =
    body === '' ||
    name.includes('\\') ||
    name.includes(':') ||
    name.startsWith('/') ||
    body.split('/').some((part) => part === '' || part === '.' || part === '..');
  if (bad) throw new Error(`unsafe zip entry name: "${name}"`);
  return name;
}

/** Writes `entries` to `zipPath` (sorted, deflated), through a `.partial` file that is renamed. */
export async function writeZip(
  entries: readonly ZipEntry[],
  zipPath: string,
  options: { readonly level?: 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 } = {},
): Promise<void> {
  const level = options.level ?? 6;
  const sorted = [...entries].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  for (const entry of sorted) safeEntryPath(entry.path);
  await mkdir(dirname(zipPath), { recursive: true });
  const partial = `${zipPath}.partial`;
  const fd = openSync(partial, 'w');
  let failure: unknown;
  try {
    const zip = new Zip((error, chunk) => {
      if (error !== null) failure ??= error;
      else writeSync(fd, chunk);
    });
    for (const entry of sorted) {
      if (entry.source.kind === 'dir') {
        const folder = new ZipPassThrough(entry.path);
        folder.mtime = STAMP;
        folder.attrs = DIRECTORY_ATTRIBUTE;
        zip.add(folder);
        folder.push(new Uint8Array(0), true);
        continue;
      }
      const bytes =
        entry.source.kind === 'bytes'
          ? entry.source.bytes
          : new Uint8Array(await readFile(entry.source.path));
      const file =
        bytes.length === 0 ? new ZipPassThrough(entry.path) : new ZipDeflate(entry.path, { level });
      file.mtime = STAMP;
      zip.add(file);
      file.push(bytes, true);
    }
    zip.end();
    if (failure !== undefined) throw failure;
  } catch (error) {
    closeSync(fd);
    rmSync(partial, { force: true });
    throw error;
  }
  closeSync(fd);
  renameSync(partial, zipPath);
}

/** Every entry of a (small) zip, by name; folders are names ending in `/` with no bytes. */
export async function readZip(zipPath: string): Promise<Record<string, Uint8Array>> {
  return unzipSync(new Uint8Array(await readFile(zipPath)));
}

/** Extracts `zipPath` into `destination`, refusing any entry that would land outside it. */
export async function extractZip(zipPath: string, destination: string): Promise<void> {
  const entries = await readZip(zipPath);
  await mkdir(destination, { recursive: true });
  for (const [name, bytes] of Object.entries(entries)) {
    safeEntryPath(name);
    const target = join(destination, ...name.split('/'));
    if (name.endsWith('/')) {
      await mkdir(target, { recursive: true });
    } else {
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, bytes);
    }
  }
}

/** Lowercase hex SHA-256 of a file, read as a stream. */
export async function sha256File(path: string): Promise<string> {
  const hasher = new Bun.CryptoHasher('sha256');
  for await (const chunk of Bun.file(path).stream()) hasher.update(chunk);
  return hasher.digest('hex');
}

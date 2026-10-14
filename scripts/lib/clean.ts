import { lstat, readdir, rm } from 'node:fs/promises';
import { join } from 'node:path';

export interface CleanOptions {
  /** Also delete every `node_modules` (a `bun install` is needed afterwards). */
  readonly deep: boolean;
  /** Leave the Cargo `target/` folder alone. */
  readonly keepTarget: boolean;
}

/** Workspace package folders one level below these hold a `package.json`. */
const PACKAGE_PARENTS = ['programs/desktop', 'programs/webapp', 'packages'] as const;

/** Per-package build output. */
const PACKAGE_OUTPUT = ['dist', 'out', 'coverage', '.turbo'] as const;

/** Names inside `release/` and `target/` that are tracked placeholders or archives. */
const KEPT_IN_RELEASE: ReadonlySet<string> = new Set(['.archive', '.gitkeep']);
const KEPT_IN_TARGET: ReadonlySet<string> = new Set(['.gitkeep']);

async function entries(dir: string): Promise<string[]> {
  try {
    return await readdir(dir);
  } catch {
    return [];
  }
}

async function exists(path: string): Promise<boolean> {
  try {
    await lstat(path);
    return true;
  } catch {
    return false;
  }
}

/**
 * Absolute paths `clean` removes under `root`: turbo and build output, `*.tsbuildinfo`, Tauri's
 * generated folders, everything in `release/` except `.archive/` and `.gitkeep`, everything in
 * `target/` except `.gitkeep` (unless `keepTarget`), and with `deep` every `node_modules`.
 * Never walks into `node_modules`, so it stays fast.
 */
export async function planClean(root: string, options: CleanOptions): Promise<string[]> {
  const found = new Set<string>();
  const add = async (path: string): Promise<void> => {
    if (await exists(path)) found.add(path);
  };

  for (const name of ['.turbo', 'out', 'coverage', ...(options.deep ? ['node_modules'] : [])]) {
    await add(join(root, name));
  }
  for (const name of await entries(root)) {
    if (name.endsWith('.tsbuildinfo')) found.add(join(root, name));
  }

  for (const parent of PACKAGE_PARENTS) {
    for (const pkg of await entries(join(root, parent))) {
      const dir = join(root, parent, pkg);
      for (const name of [...PACKAGE_OUTPUT, ...(options.deep ? ['node_modules'] : [])]) {
        await add(join(dir, name));
      }
      for (const name of await entries(dir)) {
        if (name.endsWith('.tsbuildinfo')) found.add(join(dir, name));
      }
      await add(join(dir, 'src-tauri', 'gen'));
    }
  }

  for (const name of await entries(join(root, 'release'))) {
    if (!KEPT_IN_RELEASE.has(name)) found.add(join(root, 'release', name));
  }
  if (!options.keepTarget) {
    for (const name of await entries(join(root, 'target'))) {
      if (!KEPT_IN_TARGET.has(name)) found.add(join(root, 'target', name));
    }
  }
  return [...found].sort();
}

/** Removes `paths`, returning the ones that could not be removed (for example a running exe). */
export async function removePaths(paths: readonly string[]): Promise<string[]> {
  const failed: string[] = [];
  for (const path of paths) {
    try {
      await rm(path, { recursive: true, force: true });
    } catch {
      failed.push(path);
    }
  }
  return failed;
}

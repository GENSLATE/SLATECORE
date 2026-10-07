import { join, resolve } from 'node:path';

/** Absolute path of the repository root. */
export const ROOT = resolve(import.meta.dir, '..', '..');

/** `ROOT/...segments`. */
export const fromRoot = (...segments: string[]): string => join(ROOT, ...segments);

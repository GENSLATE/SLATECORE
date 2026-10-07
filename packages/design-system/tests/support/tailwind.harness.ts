/**
 * Compiles the real `design-system.css` entry with the Tailwind v4 JS API, so tests can prove
 * that every class the components use generates CSS (Tailwind silently drops unknown names) and
 * inspect the CSS that a given set of classes produces.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { __unstable__loadDesignSystem, compile } from 'tailwindcss';

export const packageRoot = resolve(import.meta.dir, '../..');
export const srcRoot = join(packageRoot, 'src');
export const stylesRoot = join(srcRoot, 'styles');
const entryPath = join(stylesRoot, 'design-system.css');

/** Recursively lists every file under `dir`. */
export function listFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? listFiles(path) : [path];
  });
}

function resolveStylesheet(id: string, base: string): string {
  if (id.startsWith('.') || id.startsWith('/')) return resolve(base, id);
  // The bare `tailwindcss` import means its CSS entry (the `style` export condition).
  const request = id === 'tailwindcss' ? 'tailwindcss/index.css' : id;
  return Bun.resolveSync(request, base);
}

const options = {
  base: stylesRoot,
  from: entryPath,
  async loadStylesheet(id: string, base: string) {
    const path = resolveStylesheet(id, base);
    return { path, base: dirname(path), content: readFileSync(path, 'utf8') };
  },
};

/** Builds the CSS Tailwind emits for exactly these candidate classes (plus theme and base). */
export async function buildCss(candidates: string[]): Promise<string> {
  const compiler = await compile(readFileSync(entryPath, 'utf8'), options);
  return compiler.build(candidates);
}

/** Returns the candidates that generate no CSS at all (dead utilities). */
export async function deadCandidates(candidates: string[]): Promise<string[]> {
  const system = await __unstable__loadDesignSystem(readFileSync(entryPath, 'utf8'), options);
  const css = system.candidatesToCss(candidates);
  return candidates.filter((_, index) => {
    const out = css[index];
    return out == null || out.trim() === '';
  });
}

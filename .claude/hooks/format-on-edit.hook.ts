/**
 * PostToolUse (Edit, Write, MultiEdit): formats the edited file the way lefthook and CI expect,
 * Biome for TypeScript, JSON and CSS and rustfmt (edition 2024) for Rust. Whatever Biome cannot
 * fix on its own goes back to Claude as context. Files without a formatter are left alone.
 */
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

import { addContext, editedFile, outsideProject, projectDir, readInput, run } from './hook.shared';

const BIOME_FILES = /\.(?:[cm]?[jt]sx?|jsonc?|css)$/;
const MAX_REPORT_LINES = 40;

export type Formatter = 'biome' | 'rustfmt';

/** The formatter for a file, if one applies. */
export function formatterFor(relPath: string): Formatter | undefined {
  if (relPath.split('/').includes('node_modules')) return undefined;
  if (relPath.endsWith('.rs')) return 'rustfmt';
  return BIOME_FILES.test(relPath) ? 'biome' : undefined;
}

async function runCapture(
  argv: readonly string[],
  cwd: string,
): Promise<{ code: number; out: string }> {
  try {
    const child = Bun.spawn([...argv], { cwd, stdin: 'ignore', stdout: 'pipe', stderr: 'pipe' });
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    return { code, out: `${stdout}${stderr}` };
  } catch {
    return { code: 127, out: '' }; // The formatter is not installed: nothing to do.
  }
}

if (import.meta.main) {
  await run(async () => {
    const root = projectDir();
    const file = editedFile(await readInput(), root);
    if (file === undefined || outsideProject(file)) return; // Not this repository's file.
    const formatter = formatterFor(file);
    const path = resolve(root, file);
    if (formatter === undefined || !existsSync(path)) return;

    if (formatter === 'rustfmt') {
      await runCapture(['rustfmt', '--edition', '2024', path], root);
      return;
    }
    const { code, out } = await runCapture(
      [
        process.execPath,
        'x',
        '--no-install',
        'biome',
        'check',
        '--write',
        '--config-path=.config/biome.json',
        '--no-errors-on-unmatched',
        '--files-ignore-unknown=true',
        path,
      ],
      root,
    );
    if (code !== 0 && out.trim() !== '') {
      const report = out.trim().split('\n').slice(0, MAX_REPORT_LINES).join('\n');
      addContext(
        'PostToolUse',
        `Biome could not fix everything in ${file}; fix this by hand:\n${report}`,
      );
    }
  });
}

import { describe, expect, test } from 'bun:test';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');

function listFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? listFiles(path) : [path];
  });
}

describe('package boundaries', () => {
  test('package_never_imports_tauri', () => {
    const offenders = listFiles(join(root, 'src'))
      .filter((file) => /\.(ts|tsx|css)$/.test(file))
      .filter((file) => readFileSync(file, 'utf8').includes('@tauri-apps'));
    expect(offenders).toEqual([]);
  });

  test('exports_map_resolves_every_subpath', () => {
    const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')) as {
      exports: Record<string, string | { style?: string; default?: string }>;
    };
    const expected = [
      '.',
      './window',
      './layout',
      './actions',
      './inputs',
      './navigation',
      './overlays',
      './feedback',
      './display',
      './providers',
      './hooks',
      './utils',
      './theme-init',
      './design-system.css',
      './fonts.css',
    ];
    expect(Object.keys(pkg.exports).sort()).toEqual([...expected].sort());
    for (const [subpath, target] of Object.entries(pkg.exports)) {
      const files = typeof target === 'string' ? [target] : Object.values(target);
      for (const file of files) {
        expect(existsSync(join(root, file)), `${subpath} -> ${file}`).toBe(true);
      }
    }
  });
});

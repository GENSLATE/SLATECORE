import { afterEach, describe, expect, test } from 'bun:test';

import { defineWebViteConfig, WEB_BUILD_TARGET } from '../../src';

const pluginNames = async (plugins: unknown): Promise<string[]> =>
  (await Promise.all((plugins as unknown[]).map(async (plugin) => await plugin)))
    .flat(Number.POSITIVE_INFINITY)
    .filter(
      (plugin): plugin is { name: string } =>
        typeof plugin === 'object' && plugin !== null && 'name' in plugin,
    )
    .map((plugin) => plugin.name);

const TAURI_VARS = ['TAURI_ENV_PLATFORM', 'TAURI_ENV_DEBUG', 'TAURI_DEV_HOST'] as const;

afterEach(() => {
  for (const name of TAURI_VARS) delete process.env[name];
});

describe('defineWebViteConfig', () => {
  test('web_preset_has_no_tauri_env_and_sets_port', async () => {
    // A Tauri CLI environment must have no effect on a web app.
    process.env['TAURI_ENV_PLATFORM'] = 'windows';
    process.env['TAURI_ENV_DEBUG'] = 'true';
    process.env['TAURI_DEV_HOST'] = '192.168.1.20';

    const config = defineWebViteConfig({ port: 5173 });
    expect(config.server?.port).toBe(5173);
    expect(config.preview?.port).toBe(5173);
    expect(config.server?.strictPort).toBeUndefined();
    expect(config.server?.host).toBeUndefined();
    expect(config.server?.hmr).toBeUndefined();
    expect(config.server?.watch).toBeUndefined();
    expect(config.envPrefix).toBeUndefined();
    expect(config.clearScreen).toBeUndefined();
    expect(config.build?.target).toBe(WEB_BUILD_TARGET);
    expect(config.build?.minify).toBeUndefined();
    expect(config.build?.sourcemap).toBeUndefined();

    const { plugins, ...withoutPlugins } = config;
    const serialized = JSON.stringify(withoutPlugins);
    expect(serialized).not.toContain('TAURI');
    expect(serialized).not.toContain('src-tauri');
    expect(await pluginNames(plugins)).not.toContain('tauri');
  });

  test('React, React Compiler (Babel) and Tailwind plugins by default', async () => {
    const names = await pluginNames(defineWebViteConfig({ port: 5173 }).plugins);
    expect(names.some((name) => name.includes('react'))).toBe(true);
    expect(names).toContain('@rolldown/plugin-babel');
    expect(names.some((name) => name.startsWith('@tailwindcss/vite'))).toBe(true);
  });

  test('compiler and tailwind can be turned off; extra plugins and overrides apply', async () => {
    const config = defineWebViteConfig({
      port: 5174,
      reactCompiler: false,
      tailwind: false,
      plugins: [{ name: 'extra-plugin' }],
      overrides: { server: { open: false } },
    });
    const names = await pluginNames(config.plugins);
    expect(names).not.toContain('@rolldown/plugin-babel');
    expect(names.some((name) => name.startsWith('@tailwindcss/vite'))).toBe(false);
    expect(names.at(-1)).toBe('extra-plugin');
    expect(config.server?.open).toBe(false);
    expect(config.server?.port).toBe(5174);
  });

  test('rejects invalid ports', () => {
    expect(() => defineWebViteConfig({ port: 80 })).toThrow(RangeError);
    expect(() => defineWebViteConfig({ port: 5173.5 })).toThrow(RangeError);
  });
});

import babel from '@rolldown/plugin-babel';
import tailwindcss from '@tailwindcss/vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import { mergeConfig, type PluginOption, type UserConfig } from 'vite';

/** Fixed browser target for web builds (Vite's own baseline, pinned so it never drifts silently). */
export const WEB_BUILD_TARGET = 'baseline-widely-available';

export interface WebViteOptions {
  /** Dev-server and preview port for this app. */
  readonly port: number;
  /** React Compiler via Babel. Default: `true`. */
  readonly reactCompiler?: boolean;
  /** Tailwind CSS v4 Vite plugin. Default: `true`. */
  readonly tailwind?: boolean;
  /** Extra plugins, appended after the preset's. */
  readonly plugins?: readonly PluginOption[];
  /** Deep-merged over the generated config (`mergeConfig`). */
  readonly overrides?: UserConfig;
}

/**
 * The shared Vite 8 config for plain web apps (browser only, no Tauri): React (+ React Compiler),
 * Tailwind CSS v4 and a fixed browser build target. It never reads `TAURI_*` variables.
 *
 * ```ts
 * // programs/webapp/<app>/vite.config.ts
 * import { defineWebViteConfig } from '@genslate/config-vite';
 * export default defineWebViteConfig({ port: 5173 });
 * ```
 */
export function defineWebViteConfig(options: WebViteOptions): UserConfig {
  const { port, reactCompiler = true, tailwind = true } = options;
  if (!Number.isInteger(port) || port < 1024 || port > 65_535) {
    throw new RangeError(
      `defineWebViteConfig: port must be an integer in 1024..65535, got ${port}`,
    );
  }

  const plugins: PluginOption[] = [react()];
  if (reactCompiler) plugins.push(babel({ presets: [reactCompilerPreset()] }));
  if (tailwind) plugins.push(tailwindcss());
  plugins.push(...(options.plugins ?? []));

  const config: UserConfig = {
    plugins,
    server: { port },
    preview: { port },
    build: { target: WEB_BUILD_TARGET, emptyOutDir: true },
  };

  return options.overrides === undefined ? config : mergeConfig(config, options.overrides);
}

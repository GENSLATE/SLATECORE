/**
 * @genslate/config-vite — shared Vite 8 presets for GENSLATE apps:
 * React (+ React Compiler) and Tailwind CSS v4, either with the Tauri dev-server contract
 * (`defineTauriViteConfig`) or for the plain browser (`defineWebViteConfig`).
 */

export { buildTarget, type Env, readTauriEnv, type TauriEnv } from './tauri.env';
export {
  defineTauriViteConfig,
  type ReactCompilerOptions,
  type TauriViteOptions,
} from './tauri-vite.config';
export { defineWebViteConfig, WEB_BUILD_TARGET, type WebViteOptions } from './web-vite.config';

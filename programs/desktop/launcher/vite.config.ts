import { fileURLToPath } from 'node:url';

import { defineTauriViteConfig } from '@genslate/config-vite';

const page = (name: string) => fileURLToPath(new URL(name, import.meta.url));

// Port 1420 (HMR 1421), strict: it must match `build.devUrl` in src-tauri/tauri.conf.json.
// The same config serves `bun run dev` (Tauri starts Vite through beforeDevCommand) and
// `bun run dev --web` (Vite alone, the UI on the browser mock backend).
// Two pages: the launcher (`index.html`) and the tray icon's menu window (`tray-menu.html`).
export default defineTauriViteConfig({
  port: 1420,
  overrides: {
    build: {
      rolldownOptions: {
        input: { main: page('index.html'), 'tray-menu': page('tray-menu.html') },
      },
    },
  },
});

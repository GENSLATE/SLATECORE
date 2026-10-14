import { defineWebViteConfig } from '@genslate/config-vite';

// Port 1430 is the Design Kit's own. `strictPort` makes a clash fail loudly instead of moving.
// The Kit bundles every component plus the Codicon list, so it gets a looser chunk-size warning.
export default defineWebViteConfig({
  port: 1430,
  overrides: { server: { strictPort: true }, build: { chunkSizeWarningLimit: 1500 } },
});

import '@genslate/design-system/fonts.css';
import './styles/main.css';

import { WindowContextMenu } from '@genslate/design-system';
import { StrictMode, Suspense, use } from 'react';
import { createRoot } from 'react-dom/client';

import { AppProviders } from './app/app.providers';
import { type Boot, LauncherProvider, loadBoot } from './app/launcher.provider';
import { TrayMenu } from './features/tray-menu/tray-menu.component';
import { createBackend } from './ipc/launcher.client';
import type { TrayMenuAnchor } from './ipc/launcher.types';

/** The tray menu window (`tray-menu.html`): the same backend and state as the launcher. */
async function boot(): Promise<Boot & { readonly anchor: TrayMenuAnchor | undefined }> {
  const loaded = await loadBoot(await createBackend());
  // In a browser there is no tray click: open the preview straight away.
  const anchor =
    loaded.context.mode === 'web'
      ? (await import('./ipc/launcher.mock')).mockTrayAnchor()
      : undefined;
  document.documentElement.dataset['launcherMode'] = loaded.context.mode;
  return { ...loaded, anchor };
}

const booting = boot();

function Root() {
  const loaded = use(booting);
  return (
    <LauncherProvider boot={loaded}>
      <AppProviders>
        {/* No right-click menu of its own (and never the webview's): it is a menu already. */}
        <WindowContextMenu hideTheme allowNativeMenu={import.meta.env.DEV}>
          <TrayMenu initialAnchor={loaded.anchor} />
        </WindowContextMenu>
      </AppProviders>
    </LauncherProvider>
  );
}

const container = document.getElementById('root');
if (container === null) throw new Error('#root is missing from tray-menu.html');

createRoot(container).render(
  <StrictMode>
    <Suspense fallback={null}>
      <Root />
    </Suspense>
  </StrictMode>,
);

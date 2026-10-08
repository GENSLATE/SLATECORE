import '@genslate/design-system/fonts.css';
import './styles/main.css';

import { StrictMode, Suspense, use } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './app/app.component';
import { AppProviders } from './app/app.providers';
import { type Boot, LauncherProvider, loadBoot } from './app/launcher.provider';
import { createBackend } from './ipc/launcher.client';

/** Loads everything the first frame needs and hands the frame geometry to the CSS. */
async function boot(): Promise<Boot> {
  const loaded = await loadBoot(await createBackend());
  const { layout, mode } = loaded.context;
  // The shell hit-tests the frame with the same geometry the CSS draws.
  const root = document.documentElement;
  root.style.setProperty('--launcher-inset', `${layout.inset}px`);
  root.style.setProperty('--launcher-normal', `${layout.normalWidth}px`);
  root.style.setProperty('--launcher-expanded', `${layout.expandedWidth}px`);
  root.dataset['launcherMode'] = mode;
  return loaded;
}

const booting = boot();

function Root() {
  const loaded = use(booting);
  return (
    <LauncherProvider boot={loaded}>
      <AppProviders>
        <App />
      </AppProviders>
    </LauncherProvider>
  );
}

const container = document.getElementById('root');
if (container === null) throw new Error('#root is missing from index.html');

createRoot(container).render(
  <StrictMode>
    <Suspense fallback={null}>
      <Root />
    </Suspense>
  </StrictMode>,
);

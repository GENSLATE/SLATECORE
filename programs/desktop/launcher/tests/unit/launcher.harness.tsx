/**
 * Renders the whole launcher (or the tray menu) on the browser mock backend, with every backend
 * call wrapped in a bun `mock()` so tests can assert on it.
 */
import { type Mock, mock } from 'bun:test';
import { act, render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';

import { App } from '../../src/app/app.component';
import { AppProviders } from '../../src/app/app.providers';
import { type Boot, LauncherProvider } from '../../src/app/launcher.provider';
import { TrayMenu } from '../../src/features/tray-menu/tray-menu.component';
import type { LauncherBackend } from '../../src/ipc/launcher.client';
import { createMockBackend, type MockOptions } from '../../src/ipc/launcher.mock';
import type { AppList } from '../../src/ipc/launcher.types';

/** The backend with each method replaced by a spy that calls the real mock. */
export type SpiedBackend = { readonly [K in keyof LauncherBackend]: Mock<LauncherBackend[K]> };

export function spyBackend(backend: LauncherBackend): SpiedBackend {
  const entries = Object.entries(backend).map(([key, value]) => [
    key,
    mock(value as Parameters<typeof mock>[0]),
  ]);
  return Object.fromEntries(entries) as SpiedBackend;
}

export interface HarnessOptions extends MockOptions {
  /** Replaces the app list the first frame shows (e.g. every tab with zero apps). */
  readonly list?: AppList;
}

async function boot(options: HarnessOptions) {
  const { list: listOverride, ...mockOptions } = options;
  const raw = createMockBackend(mockOptions);
  const backend = spyBackend(raw);
  const [context, list, vault] = await Promise.all([
    backend.context(),
    backend.listApps(),
    backend.vaultStatus(),
  ]);
  const loaded: Boot = { backend, context, list: listOverride ?? list, vault };
  return { loaded, backend, emit: raw.emit };
}

/** Lets pending promises, effects and zero-delay timers settle inside `act`. */
export async function settle(): Promise<void> {
  await act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));
}

async function mount(loaded: Boot, children: ReactNode) {
  const view = render(
    <LauncherProvider boot={loaded}>
      <AppProviders>{children}</AppProviders>
    </LauncherProvider>,
  );
  await settle();
  return view;
}

/** The launcher window, open (the mock runs in `web` mode, so the stage opens at once). */
export async function renderLauncher(options: HarnessOptions = {}) {
  const { loaded, backend, emit } = await boot(options);
  const user = userEvent.setup();
  const view = await mount(loaded, <App />);
  return { ...view, backend, emit, user };
}

/** The tray menu window, opened at a bottom-right anchor. */
export async function renderTrayMenu(options: HarnessOptions = {}) {
  const { loaded, backend, emit } = await boot(options);
  const user = userEvent.setup();
  const view = await mount(
    loaded,
    <TrayMenu initialAnchor={{ x: 540, y: 420, opensUp: true, alignEnd: true }} />,
  );
  return { ...view, backend, emit, user };
}

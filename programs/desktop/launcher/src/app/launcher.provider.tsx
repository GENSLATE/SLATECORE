import { type ReactNode, useEffect, useState } from 'react';

import type { LauncherBackend } from '../ipc/launcher.client';
import type {
  AppList,
  LauncherContext,
  Settings,
  ShowView,
  VaultStatusDto,
} from '../ipc/launcher.types';
import { type LauncherState, LauncherStateContext, type StageState } from './launcher.context';

/** What `main.tsx` loads before the first render. */
export interface Boot {
  readonly backend: LauncherBackend;
  readonly context: LauncherContext;
  readonly list: AppList;
  readonly vault: VaultStatusDto;
}

/** Loads everything the first frame needs: context (settings, commands, layout), apps, vault. */
export async function loadBoot(backend: LauncherBackend): Promise<Boot> {
  const [context, list, vault] = await Promise.all([
    backend.context(),
    backend.listApps(),
    backend.vaultStatus(),
  ]);
  return { backend, context, list, vault };
}

/**
 * Owns the live launcher state: settings, the app list (refetched when the shell says it
 * changed), the vault state, the pinned flag and the window stage for enter/exit animations.
 */
export function LauncherProvider({ boot, children }: { boot: Boot; children: ReactNode }) {
  const { backend, context } = boot;
  const [settings, setSettings] = useState<Settings>(context.settings);
  const [list, setList] = useState<AppList>(boot.list);
  const [pinned, setPinned] = useState(context.pinned);
  const [vault, setVault] = useState<VaultStatusDto>(boot.vault);
  const [stage, setStage] = useState<StageState>('closed');
  const [showCount, setShowCount] = useState(0);
  const [showView, setShowView] = useState<ShowView>('apps');

  useEffect(() => {
    const unsubscribers: Promise<() => void>[] = [
      backend.on('shown', (view) => {
        setStage('open');
        setShowView(view);
        setShowCount((count) => count + 1);
      }),
      backend.on('willHide', () => setStage('closed')),
      backend.on('pinned', setPinned),
      backend.on('settings', setSettings),
      backend.on('vault', setVault),
      backend.on('catalog', () => {
        backend.listApps().then(setList, (error: unknown) => console.error('list_apps', error));
      }),
    ];
    // In a browser there is no shell to announce the window: open straight away.
    if (context.mode === 'web') {
      setStage('open');
      setShowCount(1);
    }
    return () => {
      for (const unsubscribe of unsubscribers) {
        unsubscribe.then(
          (stop) => stop(),
          () => undefined,
        );
      }
    };
  }, [backend, context.mode]);

  const state: LauncherState = {
    backend,
    context,
    settings,
    list,
    pinned,
    vault,
    stage,
    showCount,
    showView,
  };
  return <LauncherStateContext value={state}>{children}</LauncherStateContext>;
}

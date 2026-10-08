import { createContext, use } from 'react';

import type { LauncherBackend } from '../ipc/launcher.client';
import type {
  AppList,
  LauncherContext,
  Settings,
  ShowView,
  VaultStatusDto,
} from '../ipc/launcher.types';

/** Window presence, driven by the shell's show / will-hide events. */
export type StageState = 'open' | 'closed';

/** Shared launcher state (provided by `LauncherProvider`). */
export interface LauncherState {
  readonly backend: LauncherBackend;
  readonly context: LauncherContext;
  readonly settings: Settings;
  readonly list: AppList;
  readonly pinned: boolean;
  /** The vault's state, kept current by `vault://status`. */
  readonly vault: VaultStatusDto;
  readonly stage: StageState;
  /** Increments on every show; keys entrance animations. */
  readonly showCount: number;
  /** The view the last show asked for (the tray menu opens Help or Settings). */
  readonly showView: ShowView;
}

export const LauncherStateContext = createContext<LauncherState | null>(null);

/** The launcher state; throws outside `LauncherProvider`. */
export function useLauncher(): LauncherState {
  const state = use(LauncherStateContext);
  if (state === null) throw new Error('useLauncher() needs <LauncherProvider>');
  return state;
}

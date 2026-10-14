import { useLauncher } from '../../app/launcher.context';
import {
  type CommandHost,
  type CommandId,
  type CommandParams,
  type CommandTarget,
  runCommand,
} from '../../app/launcher-commands.model';
import type { LauncherBackend } from '../../ipc/launcher.client';
import type { AppEntry, ShowView } from '../../ipc/launcher.types';

/** The menu is failing silently otherwise: it has closed by the time a call fails. */
export function report(error: unknown): void {
  console.error('tray menu:', error);
}

/** Runs a tray row's command. */
export type TrayRun = (id: CommandId, params?: CommandParams) => void;

/** The view the shell shows the launcher on for a command's target. */
function showView(target: CommandTarget): ShowView {
  if (target.kind === 'help') return 'help';
  return target.kind === 'tool' && target.id === 'settings' ? 'settings' : 'apps';
}

/**
 * The tray window's side of the shared command registry (`launcher-commands.model.ts`): views
 * open by asking the shell to show the launcher there (a tab or a tool other than Settings
 * opens the apps); the tray has no toasts, so only failures are logged.
 */
export function trayCommandHost(
  backend: LauncherBackend,
  apps: readonly AppEntry[],
  pinned: boolean,
): CommandHost {
  return {
    backend,
    apps,
    pinned,
    tool: undefined,
    launch: (app) => {
      backend.launch(app.id).catch(report);
    },
    show: (target) => {
      backend.show(showView(target)).catch(report);
    },
    inform: () => undefined,
    fail: report,
  };
}

export function useTrayCommands(): TrayRun {
  const { backend, list, pinned } = useLauncher();
  const host = trayCommandHost(backend, list.apps, pinned);
  return (id, params) => runCommand(host, id, params);
}

/**
 * The launcher's command registry: one handler per command id, run by the slash bar (with the
 * ids of the shell's action list) and by the tray menu rows, so both do exactly the same thing.
 * What differs between the two windows comes from their `CommandHost`: the launcher window
 * switches its own view and shows toasts; the tray window asks the shell to show the launcher
 * and logs failures (its menu has closed by then).
 */
import { searchApps } from '../features/apps/catalog.model';
import { isSettingsSection, type SettingsSection } from '../features/settings/settings.model';
import type { ToolId } from '../features/tools/tools.model';
import type { LauncherBackend } from '../ipc/launcher.client';
import type {
  AppEntry,
  ConfigFile,
  SharedFolder,
  SizePreset,
  ThemeSetting,
} from '../ipc/launcher.types';

/** Where a command takes the launcher window. */
export type CommandTarget =
  | { readonly kind: 'apps' }
  | { readonly kind: 'help' }
  | { readonly kind: 'tool'; readonly id: ToolId; readonly section?: SettingsSection };

/** What a window lends its commands. */
export interface CommandHost {
  readonly backend: LauncherBackend;
  readonly apps: readonly AppEntry[];
  readonly pinned: boolean;
  /** Starts an app (the launcher plays its launch pop; the tray just asks the shell). */
  launch(app: AppEntry): void;
  /** Brings the launcher to a view: in place, or by asking the shell to show it there. */
  show(target: CommandTarget): void;
  /** Something worth telling the user. */
  inform(message: string, tone: 'info' | 'success'): void;
  /** A backend call failed. */
  fail(error: unknown): void;
}

export type CommandParams = Readonly<Record<string, string>>;

/**
 * The ids this window runs itself. Every id but `show` is an id of the shell's registry
 * (`crates/launcher-core/src/actions.rs`) and reads that registry's parameter names, so the
 * slash bar, the tray rows and the shell all mean the same thing by it. `show` is tray-only.
 * Registry ids missing here (`tab`, `fav`, `tools`, `hide`) go to the shell.
 */
export const COMMAND_IDS = [
  'open',
  'folder',
  'rescan',
  'theme',
  'size',
  'pin',
  'settings',
  'vault',
  'config',
  'keys',
  'logs',
  'ask',
  'help',
  'quit',
  'show',
] as const;

export type CommandId = (typeof COMMAND_IDS)[number];

/** `/theme` words, and the theme names the tray's Appearance menu sends. */
const THEMES: Readonly<Record<string, ThemeSetting>> = {
  dark: 'polar-night',
  light: 'snow-storm',
  system: 'system',
  'polar-night': 'polar-night',
  'snow-storm': 'snow-storm',
};
const SIZES: readonly SizePreset[] = ['s', 'm', 'l'];
const FOLDERS: readonly SharedFolder[] = [
  'desktop',
  'documents',
  'downloads',
  'music',
  'pictures',
  'videos',
  'storage',
];
const pick = <T extends string>(values: readonly T[], value: string | undefined) =>
  values.find((candidate) => candidate === value);

function openFile(host: CommandHost, file: ConfigFile): void {
  host.backend.openConfigFile(file).catch(host.fail);
}

type Handler = (host: CommandHost, params: CommandParams) => void;

const HANDLERS: Readonly<Record<CommandId, Handler>> = {
  open: (host, { app: wanted = '' }) => {
    const app = host.apps.find((entry) => entry.id === wanted) ?? searchApps(host.apps, wanted)[0];
    if (app === undefined) host.inform(`No app matches “${wanted}”`, 'info');
    else host.launch(app);
  },
  theme: (host, { mode = '' }) => {
    const theme = THEMES[mode];
    if (theme !== undefined) host.backend.setSetting('theme', theme).catch(host.fail);
  },
  size: (host, { preset }) => {
    const size = pick(SIZES, preset);
    if (size !== undefined) host.backend.setSetting('size', size).catch(host.fail);
  },
  pin: (host) => {
    host.backend.setPinned(!host.pinned).catch(host.fail);
  },
  settings: (host, { section }) =>
    host.show({
      kind: 'tool',
      id: 'settings',
      section: isSettingsSection(section) ? section : 'appearance',
    }),
  vault: (host) => host.show({ kind: 'tool', id: 'settings', section: 'vault' }),
  rescan: (host) => {
    host.backend.rescan().then(() => host.inform('Apps rescanned', 'success'), host.fail);
  },
  help: (host) => host.show({ kind: 'help' }),
  ask: (host) => host.show({ kind: 'tool', id: 'ai' }),
  show: (host) => host.show({ kind: 'apps' }),
  folder: (host, { name }) => {
    const target = pick(FOLDERS, name);
    if (target !== undefined) host.backend.openFolder(target).catch(host.fail);
  },
  config: (host) => openFile(host, 'settings'),
  keys: (host) => openFile(host, 'keybindings'),
  logs: (host) => openFile(host, 'logs'),
  quit: (host) => {
    host.backend.quit().catch(host.fail);
  },
};

function isCommandId(id: string): id is CommandId {
  return COMMAND_IDS.some((known) => known === id);
}

/** Runs command `id`; an id the launcher has no handler for goes to the shell (`run_action`). */
export function runCommand(host: CommandHost, id: string, params: CommandParams = {}): void {
  if (isCommandId(id)) {
    HANDLERS[id](host, params);
    return;
  }
  host.backend.runAction(id, params).then((outcome) => {
    if (outcome.kind === 'done' && outcome.message !== null)
      host.inform(outcome.message, 'success');
  }, host.fail);
}

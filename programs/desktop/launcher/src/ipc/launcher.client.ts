/**
 * The launcher backend: typed calls into `src-tauri/src/commands.rs` through
 * `@genslate/tauri-bridge`, or an in-memory mock when the UI runs in a plain browser
 * (`bun run dev --web`) so it can be designed and screenshot anywhere.
 *
 * Every payload that comes back is checked by `launcher.parse.ts`, so a malformed one rejects
 * with a `PayloadError` instead of breaking the component that awaited it. Vault commands carry
 * passwords: this layer never logs their payloads.
 */
import { customSchemeUrl, invokeCommand, isTauri, listenEvent } from '@genslate/tauri-bridge';

import {
  parseActionOutcome,
  parseAppList,
  parseContext,
  parseEntries,
  parseLockReport,
  parseSettings,
  parseShowView,
  parseTelemetry,
  parseTrayMenuAnchor,
  parseVaultOpened,
  parseVaultStatus,
  parseVerifyReport,
  parseVolume,
} from './launcher.parse';
import {
  type ActionOutcome,
  type AppList,
  type ConfigFile,
  type EntryDto,
  type LauncherContext,
  type LauncherEvents,
  type LockReportDto,
  type OverridePatch,
  type SettingKey,
  type SharedFolder,
  type ShowView,
  VAULT_COMMANDS,
  type VaultConflict,
  type VaultStatusDto,
  type VerifyReportDto,
  type VolumeInfo,
} from './launcher.types';

/** The vault half of the backend (research/vault.md §8.3). */
export interface VaultBackend {
  vaultStatus(): Promise<VaultStatusDto>;
  /** The UI confirms the password twice first; leaves the vault unlocked. */
  vaultCreate(password: string): Promise<VaultStatusDto>;
  vaultUnlock(password: string): Promise<VaultStatusDto>;
  vaultLock(force?: boolean): Promise<LockReportDto>;
  vaultChangePassword(current: string, next: string): Promise<VaultStatusDto>;
  /** `''` is the root. */
  vaultList(dir: string): Promise<readonly EntryDto[]>;
  /** `sources` are OS paths; the originals are kept. */
  vaultImport(
    sources: readonly string[],
    destDir: string,
    onConflict: VaultConflict,
  ): Promise<readonly EntryDto[]>;
  vaultExport(path: string, destPath: string): Promise<void>;
  /** Decrypts into the session folder and opens it with the default app. */
  vaultOpen(path: string): Promise<{ readonly sessionPath: string }>;
  vaultVerify(): Promise<VerifyReportDto>;
}

/** Everything the UI can ask of the shell. */
export interface LauncherBackend extends VaultBackend {
  context(): Promise<LauncherContext>;
  listApps(): Promise<AppList>;
  rescan(): Promise<void>;
  launch(id: string, args?: readonly string[]): Promise<void>;
  openAppFolder(id: string): Promise<void>;
  setOverride(id: string, patch: OverridePatch): Promise<void>;
  openFolder(folder: SharedFolder): Promise<void>;
  openConfigFile(file: ConfigFile): Promise<void>;
  setSetting(key: SettingKey, value: string): Promise<void>;
  volume(): Promise<VolumeInfo | null>;
  setTelemetryActive(active: boolean): Promise<void>;
  setPinned(pinned: boolean): Promise<void>;
  /** Widens (or shrinks) the shell's hit area to the frame width. */
  setExpanded(expanded: boolean): Promise<void>;
  setPopupOpen(open: boolean): Promise<void>;
  hide(): Promise<void>;
  /** Shows the launcher window on `view` (from the tray menu). */
  show(view?: ShowView): Promise<void>;
  /** Hides the tray menu window (after its exit animation). */
  hideTrayMenu(): Promise<void>;
  quit(): Promise<void>;
  runAction(id: string, params: Readonly<Record<string, string>>): Promise<ActionOutcome>;
  on<E extends keyof LauncherEvents>(
    event: E,
    handler: (payload: LauncherEvents[E]) => void,
  ): Promise<() => void>;
  iconUrl(id: string): string;
}

const EVENT_NAMES: { readonly [E in keyof LauncherEvents]: string } = {
  shown: 'launcher://shown',
  willHide: 'launcher://will-hide',
  pinned: 'launcher://pinned',
  settings: 'launcher://settings',
  catalog: 'launcher://catalog',
  telemetry: 'launcher://telemetry',
  vault: 'vault://status',
  trayMenuOpen: 'tray-menu://open',
  trayMenuClose: 'tray-menu://close',
};

/** Payload parsers per event (events with no payload pass `undefined`). */
const EVENT_PARSERS: { readonly [E in keyof LauncherEvents]: (raw: unknown) => LauncherEvents[E] } =
  {
    shown: parseShowView,
    willHide: () => undefined,
    pinned: (raw) => raw === true,
    settings: parseSettings,
    catalog: () => undefined,
    telemetry: parseTelemetry,
    vault: parseVaultStatus,
    trayMenuOpen: parseTrayMenuAnchor,
    trayMenuClose: () => undefined,
  };

const tauriBackend: LauncherBackend = {
  context: async () => parseContext(await invokeCommand<unknown>('get_context')),
  listApps: async () => parseAppList(await invokeCommand<unknown>('list_apps')),
  rescan: () => invokeCommand('rescan'),
  launch: (id, args) => invokeCommand('launch_app', { id, args: args ?? null }),
  openAppFolder: (id) => invokeCommand('open_app_folder', { id }),
  setOverride: (id, patch) => invokeCommand('set_app_override', { id, patch }),
  openFolder: (folder) => invokeCommand('open_folder', { folder }),
  openConfigFile: (file) => invokeCommand('open_config_file', { file }),
  setSetting: (key, value) => invokeCommand('set_setting', { key, value }),
  volume: async () => parseVolume(await invokeCommand<unknown>('get_volume_info')),
  setTelemetryActive: (active) => invokeCommand('set_telemetry_active', { active }),
  setPinned: (pinned) => invokeCommand('window_set_pinned', { pinned }),
  setExpanded: (expanded) => invokeCommand('window_set_expanded', { expanded }),
  setPopupOpen: (open) => invokeCommand('window_set_popup_open', { open }),
  hide: () => invokeCommand('window_hide'),
  show: (view) => invokeCommand('window_show', { view: view ?? 'apps' }),
  hideTrayMenu: () => invokeCommand('tray_menu_hide'),
  quit: () => invokeCommand('quit'),
  runAction: async (id, params) =>
    parseActionOutcome(await invokeCommand<unknown>('run_action', { id, params })),
  on: (event, handler) =>
    listenEvent<unknown>(EVENT_NAMES[event], (raw) => handler(EVENT_PARSERS[event](raw))),
  iconUrl: (id) => customSchemeUrl('launcher-icon', id.split('/')),

  vaultStatus: async () => parseVaultStatus(await invokeCommand<unknown>(VAULT_COMMANDS.status)),
  vaultCreate: async (password) =>
    parseVaultStatus(await invokeCommand<unknown>(VAULT_COMMANDS.create, { password })),
  vaultUnlock: async (password) =>
    parseVaultStatus(await invokeCommand<unknown>(VAULT_COMMANDS.unlock, { password })),
  vaultLock: async (force) =>
    parseLockReport(
      await invokeCommand<unknown>(VAULT_COMMANDS.lock, force === undefined ? {} : { force }),
    ),
  vaultChangePassword: async (current, next) =>
    parseVaultStatus(
      await invokeCommand<unknown>(VAULT_COMMANDS.changePassword, { current, next }),
    ),
  vaultList: async (dir) =>
    parseEntries(await invokeCommand<unknown>(VAULT_COMMANDS.list, { dir })),
  vaultImport: async (sources, destDir, onConflict) =>
    parseEntries(
      await invokeCommand<unknown>(VAULT_COMMANDS.import, { sources, destDir, onConflict }),
    ),
  vaultExport: (path, destPath) => invokeCommand(VAULT_COMMANDS.export, { path, destPath }),
  vaultOpen: async (path) =>
    parseVaultOpened(await invokeCommand<unknown>(VAULT_COMMANDS.open, { path })),
  vaultVerify: async () =>
    parseVerifyReport(await invokeCommand<unknown>(VAULT_COMMANDS.verify, {})),
};

/** The Tauri backend inside the desktop app, the mock (with its icon set) in a browser. */
export async function createBackend(): Promise<LauncherBackend> {
  if (isTauri()) return tauriBackend;
  const [{ createMockBackend, mockOptionsFromUrl }, { MOCK_ICONS }] = await Promise.all([
    import('./launcher.mock'),
    import('./launcher.mock-icons'),
  ]);
  return createMockBackend({ ...mockOptionsFromUrl(), icons: MOCK_ICONS });
}

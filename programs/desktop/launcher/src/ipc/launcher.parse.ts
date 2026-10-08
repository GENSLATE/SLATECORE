/**
 * Boundary checks for IPC payloads: cheap structural validation so a mismatched Rust/TS pair
 * fails loudly at the edge instead of deep inside a component.
 */
import type {
  AppEntry,
  AppList,
  EntryDto,
  LauncherContext,
  Settings,
  ShowView,
  TabInfo,
  Telemetry,
  TrayMenuAnchor,
  VaultErrorDto,
  VaultStatusDto,
  VolumeInfo,
} from './launcher.types';

/** Thrown when a payload doesn't have the expected shape. */
export class PayloadError extends Error {
  constructor(what: string) {
    super(`Unexpected ${what} from the launcher backend`);
    this.name = 'PayloadError';
  }
}

type Json = Record<string, unknown>;

function isObject(value: unknown): value is Json {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function array(value: unknown, what: string): readonly unknown[] {
  if (!Array.isArray(value)) throw new PayloadError(what);
  return value;
}

function isAppEntry(value: unknown): value is AppEntry {
  return (
    isObject(value) &&
    typeof value['id'] === 'string' &&
    typeof value['name'] === 'string' &&
    typeof value['status'] === 'string' &&
    typeof value['favorite'] === 'boolean' &&
    Array.isArray(value['keywords'])
  );
}

function isTab(value: unknown): value is TabInfo {
  return (
    isObject(value) && typeof value['source'] === 'string' && typeof value['count'] === 'number'
  );
}

export function parseAppList(value: unknown): AppList {
  if (!isObject(value)) throw new PayloadError('app list');
  const apps = array(value['apps'], 'apps');
  if (!apps.every(isAppEntry)) throw new PayloadError('app entry');
  const tabs = array(value['tabs'], 'tabs');
  const recent = array(value['recent'], 'recent apps');
  if (!tabs.every(isTab)) throw new PayloadError('tab');
  return {
    apps,
    tabs,
    recent: recent.filter((id): id is string => typeof id === 'string'),
  };
}

function isContext(value: unknown): value is LauncherContext {
  if (!isObject(value)) return false;
  const { settings, layout, actions, version, mode } = value;
  return (
    typeof version === 'string' &&
    (mode === 'suite' || mode === 'dev' || mode === 'web') &&
    Array.isArray(actions) &&
    isObject(layout) &&
    typeof layout['inset'] === 'number' &&
    typeof layout['normalWidth'] === 'number' &&
    typeof layout['expandedWidth'] === 'number' &&
    isSettings(settings)
  );
}

export function parseContext(value: unknown): LauncherContext {
  if (!isContext(value)) throw new PayloadError('context');
  return value;
}

function isVolume(value: unknown): value is VolumeInfo {
  return (
    isObject(value) &&
    typeof value['label'] === 'string' &&
    typeof value['totalBytes'] === 'number' &&
    typeof value['availableBytes'] === 'number'
  );
}

export function parseVolume(value: unknown): VolumeInfo | null {
  if (value === null) return null;
  if (!isVolume(value)) throw new PayloadError('volume');
  return value;
}

const TELEMETRY_KEYS = [
  'cpuTempC',
  'gpuTempC',
  'cpuUsagePct',
  'gpuUsagePct',
  'netDownBps',
  'netUpBps',
] as const;

export function parseTelemetry(value: unknown): Telemetry {
  if (!isObject(value)) throw new PayloadError('telemetry');
  const read = (key: (typeof TELEMETRY_KEYS)[number]) => {
    const reading = value[key];
    return typeof reading === 'number' && Number.isFinite(reading) ? reading : null;
  };
  return {
    cpuTempC: read('cpuTempC'),
    gpuTempC: read('gpuTempC'),
    cpuUsagePct: read('cpuUsagePct'),
    gpuUsagePct: read('gpuUsagePct'),
    netDownBps: read('netDownBps'),
    netUpBps: read('netUpBps'),
  };
}

function isSettings(value: unknown): value is Settings {
  return isObject(value) && isObject(value['config']) && isObject(value['keybindings']);
}

export function parseSettings(value: unknown): Settings {
  if (!isSettings(value)) throw new PayloadError('settings');
  return value;
}

export function parseShowView(value: unknown): ShowView {
  return value === 'help' || value === 'settings' ? value : 'apps';
}

export function parseTrayMenuAnchor(value: unknown): TrayMenuAnchor {
  if (!isObject(value)) throw new PayloadError('tray menu anchor');
  const { x, y, opensUp, alignEnd } = value;
  if (
    typeof x !== 'number' ||
    typeof y !== 'number' ||
    typeof opensUp !== 'boolean' ||
    typeof alignEnd !== 'boolean'
  )
    throw new PayloadError('tray menu anchor');
  return { x, y, opensUp, alignEnd };
}

export function parseVaultStatus(value: unknown): VaultStatusDto {
  if (
    !isObject(value) ||
    (value['state'] !== 'uninitialized' &&
      value['state'] !== 'locked' &&
      value['state'] !== 'unlocked') ||
    typeof value['failedAttempts'] !== 'number'
  )
    throw new PayloadError('vault status');
  return value as unknown as VaultStatusDto;
}

function isEntry(value: unknown): value is EntryDto {
  return (
    isObject(value) &&
    typeof value['path'] === 'string' &&
    (value['kind'] === 'file' || value['kind'] === 'dir') &&
    typeof value['size'] === 'number'
  );
}

export function parseEntries(value: unknown): readonly EntryDto[] {
  const entries = array(value, 'vault entries');
  if (!entries.every(isEntry)) throw new PayloadError('vault entry');
  return entries;
}

/** A rejected vault command (`VaultErrorDto`); its message never contains the password. */
export function isVaultError(value: unknown): value is VaultErrorDto {
  return (
    isObject(value) && typeof value['code'] === 'string' && typeof value['message'] === 'string'
  );
}

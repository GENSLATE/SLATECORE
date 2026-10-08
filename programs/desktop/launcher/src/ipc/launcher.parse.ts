/**
 * Boundary checks for IPC payloads. Every field of every payload is checked and copied into a
 * fresh object, so a mismatched Rust/TS pair fails loudly at the edge (a `PayloadError` that
 * names the field) instead of deep inside a component, and unknown fields never leak through.
 */
import type {
  ActionOutcome,
  ActionSpec,
  AppEntry,
  AppList,
  AppStatus,
  Effect,
  EntryDto,
  FrameLayout,
  KdfParamsDto,
  Keybindings,
  LauncherConfig,
  LauncherContext,
  LockReportDto,
  ParamSpec,
  RunMode,
  Settings,
  ShowView,
  SizePreset,
  Source,
  StatusMode,
  TabInfo,
  Telemetry,
  ThemeSetting,
  TrayMenuAnchor,
  VaultErrorCode,
  VaultErrorDto,
  VaultProblem,
  VaultState,
  VaultStatusDto,
  VerifyReportDto,
  VolumeInfo,
} from './launcher.types';

/** Thrown when a payload doesn't have the expected shape. */
export class PayloadError extends Error {
  constructor(what: string) {
    super(`Unexpected ${what} from the launcher backend`);
    this.name = 'PayloadError';
  }
}

type Json = Readonly<Record<string, unknown>>;

function isObject(value: unknown): value is Json {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function object(value: unknown, what: string): Json {
  if (!isObject(value)) throw new PayloadError(what);
  return value;
}

function array<T>(value: unknown, what: string, item: (raw: unknown, what: string) => T): T[] {
  if (!Array.isArray(value)) throw new PayloadError(what);
  return value.map((raw, index) => item(raw, `${what}[${index}]`));
}

function string(value: unknown, what: string): string {
  if (typeof value !== 'string') throw new PayloadError(what);
  return value;
}

function number(value: unknown, what: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new PayloadError(what);
  return value;
}

function boolean(value: unknown, what: string): boolean {
  if (typeof value !== 'boolean') throw new PayloadError(what);
  return value;
}

function nullable<T>(
  read: (value: unknown, what: string) => T,
): (value: unknown, what: string) => T | null {
  return (value, what) => (value === null ? null : read(value, what));
}

/** A reader for one of `values` (a string union). */
function oneOf<T extends string>(values: readonly T[]): (value: unknown, what: string) => T {
  return (value, what) => {
    const match = values.find((candidate) => candidate === value);
    if (match === undefined) throw new PayloadError(what);
    return match;
  };
}

const strings = (value: unknown, what: string) => array(value, what, string);
const nullableString = nullable(string);
const nullableNumber = nullable(number);

const SOURCES: readonly Source[] = ['genslate', 'portableapps', 'portapps'];
const APP_STATUSES: readonly AppStatus[] = [
  'ready',
  'running',
  'not-installed',
  'missing-exe',
  'broken-manifest',
];
const THEMES: readonly ThemeSetting[] = ['system', 'polar-night', 'snow-storm'];
const SIZES: readonly SizePreset[] = ['s', 'm', 'l'];
const STATUS_MODES: readonly StatusMode[] = ['temps', 'usage'];
const RUN_MODES: readonly RunMode[] = ['suite', 'dev', 'web'];
const EFFECTS: readonly Effect[] = [
  'ui',
  'window',
  'read',
  'launch',
  'open',
  'writes-config',
  'ai',
];
const VAULT_STATES: readonly VaultState[] = ['uninitialized', 'locked', 'unlocked'];
const ENTRY_KINDS: readonly EntryDto['kind'][] = ['file', 'dir'];
const VAULT_PROBLEMS: readonly VaultProblem[] = [
  'blobMissing',
  'tampered',
  'truncated',
  'sizeMismatch',
];
const VAULT_ERROR_CODES: readonly VaultErrorCode[] = [
  'UNINITIALIZED',
  'ALREADY_EXISTS',
  'LOCKED',
  'ALREADY_UNLOCKED',
  'WRONG_PASSWORD',
  'THROTTLED',
  'PASSWORD_REJECTED',
  'BUSY',
  'HEADER_DAMAGED',
  'UNSUPPORTED_VERSION',
  'UNSUPPORTED_KDF',
  'OUT_OF_MEMORY',
  'TAMPERED',
  'BLOB_MISSING',
  'NOT_FOUND',
  'EXISTS',
  'INVALID_PATH',
  'LIMIT_EXCEEDED',
  'FILE_TOO_LARGE',
  'CANCELLED',
  'UNSYNCED_EDITS',
  'IO',
  'INTERNAL',
];

// ── Apps ──────────────────────────────────────────────────────────────────────

function appEntry(value: unknown, what: string): AppEntry {
  const raw = object(value, what);
  return {
    id: string(raw['id'], `${what}.id`),
    name: string(raw['name'], `${what}.name`),
    description: string(raw['description'], `${what}.description`),
    category: string(raw['category'], `${what}.category`),
    color: nullableString(raw['color'], `${what}.color`),
    keywords: strings(raw['keywords'], `${what}.keywords`),
    version: nullableString(raw['version'], `${what}.version`),
    publisher: nullableString(raw['publisher'], `${what}.publisher`),
    status: oneOf(APP_STATUSES)(raw['status'], `${what}.status`),
    favorite: boolean(raw['favorite'], `${what}.favorite`),
    hidden: boolean(raw['hidden'], `${what}.hidden`),
    args: strings(raw['args'], `${what}.args`),
    hasIcon: boolean(raw['hasIcon'], `${what}.hasIcon`),
  };
}

function tabInfo(value: unknown, what: string): TabInfo {
  const raw = object(value, what);
  return {
    source: oneOf(SOURCES)(raw['source'], `${what}.source`),
    label: string(raw['label'], `${what}.label`),
    count: number(raw['count'], `${what}.count`),
  };
}

export function parseAppList(value: unknown): AppList {
  const raw = object(value, 'app list');
  return {
    apps: array(raw['apps'], 'apps', appEntry),
    tabs: array(raw['tabs'], 'tabs', tabInfo),
    recent: strings(raw['recent'], 'recent apps'),
  };
}

// ── Settings and context ──────────────────────────────────────────────────────

function launcherConfig(value: unknown, what: string): LauncherConfig {
  const raw = object(value, what);
  const appearance = object(raw['appearance'], `${what}.appearance`);
  const behavior = object(raw['behavior'], `${what}.behavior`);
  const status = object(raw['status'], `${what}.status`);
  return {
    appearance: {
      theme: oneOf(THEMES)(appearance['theme'], `${what}.appearance.theme`),
      size: oneOf(SIZES)(appearance['size'], `${what}.appearance.size`),
    },
    behavior: {
      hideOnBlur: boolean(behavior['hideOnBlur'], `${what}.behavior.hideOnBlur`),
      hideOnLaunch: boolean(behavior['hideOnLaunch'], `${what}.behavior.hideOnLaunch`),
      pinned: boolean(behavior['pinned'], `${what}.behavior.pinned`),
    },
    status: { mode: oneOf(STATUS_MODES)(status['mode'], `${what}.status.mode`) },
  };
}

function keybindings(value: unknown, what: string): Keybindings {
  const raw = object(value, what);
  const global = object(raw['global'], `${what}.global`);
  const launcher = object(raw['launcher'], `${what}.launcher`);
  const key = (name: string) => string(launcher[name], `${what}.launcher.${name}`);
  return {
    global: { toggle: string(global['toggle'], `${what}.global.toggle`) },
    launcher: {
      focusSearch: key('focusSearch'),
      toggleTools: key('toggleTools'),
      togglePin: key('togglePin'),
      toggleFavorite: key('toggleFavorite'),
      tabGenslate: key('tabGenslate'),
      tabPortableapps: key('tabPortableapps'),
      tabPortapps: key('tabPortapps'),
    },
  };
}

function settings(value: unknown, what: string): Settings {
  const raw = object(value, what);
  return {
    config: launcherConfig(raw['config'], `${what}.config`),
    keybindings: keybindings(raw['keybindings'], `${what}.keybindings`),
    issue: nullableString(raw['issue'], `${what}.issue`),
  };
}

export function parseSettings(value: unknown): Settings {
  return settings(value, 'settings');
}

function paramSpec(value: unknown, what: string): ParamSpec {
  const raw = object(value, what);
  const base = {
    name: string(raw['name'], `${what}.name`),
    description: string(raw['description'], `${what}.description`),
    required: boolean(raw['required'], `${what}.required`),
  };
  const type = oneOf(['app', 'choice', 'text'] as const)(raw['type'], `${what}.type`);
  return type === 'choice'
    ? { ...base, type, values: strings(raw['values'], `${what}.values`) }
    : { ...base, type };
}

function actionSpec(value: unknown, what: string): ActionSpec {
  const raw = object(value, what);
  return {
    id: string(raw['id'], `${what}.id`),
    title: string(raw['title'], `${what}.title`),
    description: string(raw['description'], `${what}.description`),
    params: array(raw['params'], `${what}.params`, paramSpec),
    effect: oneOf(EFFECTS)(raw['effect'], `${what}.effect`),
  };
}

function frameLayout(value: unknown, what: string): FrameLayout {
  const raw = object(value, what);
  return {
    inset: number(raw['inset'], `${what}.inset`),
    normalWidth: number(raw['normalWidth'], `${what}.normalWidth`),
    expandedWidth: number(raw['expandedWidth'], `${what}.expandedWidth`),
  };
}

export function parseContext(value: unknown): LauncherContext {
  const raw = object(value, 'context');
  return {
    version: string(raw['version'], 'context.version'),
    mode: oneOf(RUN_MODES)(raw['mode'], 'context.mode'),
    suiteName: nullableString(raw['suiteName'], 'context.suiteName'),
    profile: string(raw['profile'], 'context.profile'),
    settings: settings(raw['settings'], 'context.settings'),
    pinned: boolean(raw['pinned'], 'context.pinned'),
    actions: array(raw['actions'], 'context.actions', actionSpec),
    layout: frameLayout(raw['layout'], 'context.layout'),
  };
}

/** What `run_action` reports back. */
export function parseActionOutcome(value: unknown): ActionOutcome {
  const raw = object(value, 'action outcome');
  const kind = oneOf(['done', 'ui'] as const)(raw['kind'], 'action outcome.kind');
  return kind === 'done'
    ? { kind, message: nullableString(raw['message'], 'action outcome.message') }
    : { kind, id: string(raw['id'], 'action outcome.id') };
}

// ── Window, drive and tray ────────────────────────────────────────────────────

export function parseVolume(value: unknown): VolumeInfo | null {
  if (value === null) return null;
  const raw = object(value, 'volume');
  return {
    label: string(raw['label'], 'volume.label'),
    name: nullableString(raw['name'], 'volume.name'),
    totalBytes: number(raw['totalBytes'], 'volume.totalBytes'),
    availableBytes: number(raw['availableBytes'], 'volume.availableBytes'),
    removable: boolean(raw['removable'], 'volume.removable'),
  };
}

/** Readings that are missing or not finite numbers are "not available here" (`null`). */
export function parseTelemetry(value: unknown): Telemetry {
  const raw = object(value, 'telemetry');
  const read = (key: keyof Telemetry) => {
    const reading = raw[key];
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

export function parseShowView(value: unknown): ShowView {
  return value === 'help' || value === 'settings' ? value : 'apps';
}

export function parseTrayMenuAnchor(value: unknown): TrayMenuAnchor {
  const raw = object(value, 'tray menu anchor');
  return {
    x: number(raw['x'], 'tray menu anchor.x'),
    y: number(raw['y'], 'tray menu anchor.y'),
    opensUp: boolean(raw['opensUp'], 'tray menu anchor.opensUp'),
    alignEnd: boolean(raw['alignEnd'], 'tray menu anchor.alignEnd'),
  };
}

// ── Vault ─────────────────────────────────────────────────────────────────────

function kdfParams(value: unknown, what: string): KdfParamsDto {
  const raw = object(value, what);
  return {
    mCostKib: number(raw['mCostKib'], `${what}.mCostKib`),
    tCost: number(raw['tCost'], `${what}.tCost`),
    pCost: number(raw['pCost'], `${what}.pCost`),
  };
}

export function parseVaultStatus(value: unknown): VaultStatusDto {
  const raw = object(value, 'vault status');
  return {
    state: oneOf(VAULT_STATES)(raw['state'], 'vault status.state'),
    kdf: nullable(kdfParams)(raw['kdf'], 'vault status.kdf'),
    kdfUpgradePending: boolean(raw['kdfUpgradePending'], 'vault status.kdfUpgradePending'),
    failedAttempts: number(raw['failedAttempts'], 'vault status.failedAttempts'),
    retryAfterMs: nullableNumber(raw['retryAfterMs'], 'vault status.retryAfterMs'),
    foreignItems: number(raw['foreignItems'], 'vault status.foreignItems'),
    entryCount: nullableNumber(raw['entryCount'], 'vault status.entryCount'),
    sessionFiles: number(raw['sessionFiles'], 'vault status.sessionFiles'),
  };
}

function entry(value: unknown, what: string): EntryDto {
  const raw = object(value, what);
  return {
    path: string(raw['path'], `${what}.path`),
    kind: oneOf(ENTRY_KINDS)(raw['kind'], `${what}.kind`),
    size: number(raw['size'], `${what}.size`),
    modifiedMs: number(raw['modifiedMs'], `${what}.modifiedMs`),
  };
}

export function parseEntries(value: unknown): readonly EntryDto[] {
  return array(value, 'vault entries', entry);
}

export function parseLockReport(value: unknown): LockReportDto {
  const raw = object(value, 'lock report');
  return {
    synced: strings(raw['synced'], 'lock report.synced'),
    unsynced: strings(raw['unsynced'], 'lock report.unsynced'),
    wipedFiles: number(raw['wipedFiles'], 'lock report.wipedFiles'),
    undeletable: strings(raw['undeletable'], 'lock report.undeletable'),
  };
}

export function parseVerifyReport(value: unknown): VerifyReportDto {
  const raw = object(value, 'verify report');
  return {
    filesChecked: number(raw['filesChecked'], 'verify report.filesChecked'),
    orphanBlobs: number(raw['orphanBlobs'], 'verify report.orphanBlobs'),
    problems: array(raw['problems'], 'verify report.problems', (item, what) => {
      const problem = object(item, what);
      return {
        path: string(problem['path'], `${what}.path`),
        problem: oneOf(VAULT_PROBLEMS)(problem['problem'], `${what}.problem`),
      };
    }),
  };
}

/** What `vault_open` returns: where the decrypted copy was put. */
export function parseVaultOpened(value: unknown): { readonly sessionPath: string } {
  const raw = object(value, 'opened vault file');
  return { sessionPath: string(raw['sessionPath'], 'opened vault file.sessionPath') };
}

/** A rejected vault command (`VaultErrorDto`); its message never contains the password. */
export function isVaultError(value: unknown): value is VaultErrorDto {
  if (!isObject(value) || typeof value['message'] !== 'string') return false;
  if (!VAULT_ERROR_CODES.some((code) => code === value['code'])) return false;
  const { retryAfterMs, paths } = value;
  return (
    (retryAfterMs === undefined ||
      (typeof retryAfterMs === 'number' && Number.isFinite(retryAfterMs))) &&
    (paths === undefined ||
      (Array.isArray(paths) && paths.every((path) => typeof path === 'string')))
  );
}

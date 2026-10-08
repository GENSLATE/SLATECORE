/**
 * In-memory launcher backend for a plain browser (`bun run dev --web`) and the unit tests: a
 * plausible drive (apps from all three sources, favorites, recent and unavailable apps, a drive
 * with its free space), drifting telemetry and an in-memory vault, so the UI can be designed,
 * tested and screenshot without the desktop shell. Loaded lazily, never part of the desktop
 * bundle's startup path. The icon set is passed in (`launcher.mock-icons.ts`, Vite only).
 */
import type { LauncherBackend } from './launcher.client';
import { PREVIEW_VAULT_PASSWORD } from './launcher.preview';
import type {
  ActionSpec,
  AppEntry,
  AppStatus,
  EntryDto,
  LauncherContext,
  LauncherEvents,
  Settings,
  SizePreset,
  Source,
  StatusMode,
  TabInfo,
  Telemetry,
  ThemeSetting,
  TrayMenuAnchor,
  VaultErrorCode,
  VaultErrorDto,
  VaultState,
  VaultStatusDto,
} from './launcher.types';

/** The demo vault's password (the preview shows it as a hint). */
export const MOCK_VAULT_PASSWORD = PREVIEW_VAULT_PASSWORD;

export interface MockOptions {
  /** Replaces the whole catalog (e.g. `[]` for an empty drive). */
  readonly apps?: readonly AppEntry[] | undefined;
  readonly recent?: readonly string[] | undefined;
  /** Only GENSLATE apps (shows the single-source heading). */
  readonly noThirdParty?: boolean | undefined;
  readonly theme?: ThemeSetting | undefined;
  readonly size?: SizePreset | undefined;
  readonly pinned?: boolean | undefined;
  readonly status?: StatusMode | undefined;
  /** Initial vault state. @default 'locked' */
  readonly vault?: VaultState | undefined;
  /**
   * Browser preview niceties: hiding comes back after a moment, the tray menu reopens, vault
   * calls take as long as a real key derivation. Off in tests.
   */
  readonly preview?: boolean | undefined;
  /** GENSLATE icons by app key (raw SVG). */
  readonly icons?: Readonly<Record<string, string>> | undefined;
}

export interface MockBackend extends LauncherBackend {
  /** Emits a shell event (tests and the preview use it to play the shell's part). */
  emit<E extends keyof LauncherEvents>(event: E, payload: LauncherEvents[E]): void;
}

type Seed = readonly [
  key: string,
  name: string,
  description: string,
  category: string,
  status: AppStatus,
  color?: string,
];

/** SLATECORE apps by GENSLATE (`programs/genslate/<app>/`). */
const GENSLATE: readonly Seed[] = [
  ['explorer', 'Explorer', 'Browse, search and manage your files', 'System', 'running', 'nord8'],
  [
    'terminal',
    'Terminal',
    'A fast terminal with tabs and splits',
    'Development',
    'ready',
    'nord14',
  ],
  ['coder', 'Coder', 'A focused code editor for your projects', 'Development', 'ready', 'nord10'],
  ['editor', 'Editor', 'Notes, plain text and Markdown', 'Office', 'running', 'nord9'],
  ['browser', 'Browser', 'A private, portable web browser', 'Internet', 'ready', 'nord7'],
  ['gallery', 'Gallery', 'View and organise your pictures', 'Media', 'ready', 'nord15'],
  ['jukebox', 'Jukebox', 'Play your music library and playlists', 'Media', 'ready', 'nord11'],
  ['theater', 'Theater', 'Watch videos and films', 'Media', 'missing-exe', 'nord12'],
  [
    'toolbox',
    'Toolbox',
    'Handy utilities for everyday tasks',
    'Utilities',
    'not-installed',
    'nord13',
  ],
  ['aistudio', 'AI Studio', 'Chat, create and automate with AI', 'AI', 'not-installed', 'nord10'],
];

/** portapps.io apps (`programs/portapps.io/<app>-portable/`). */
const PORTAPPS: readonly Seed[] = [
  ['vscodium-portable', 'VSCodium', 'Code editor without telemetry', 'Development', 'ready'],
  ['brave-portable', 'Brave', 'Privacy-focused web browser', 'Internet', 'ready'],
  ['phyrox-portable', 'Phyrox', 'Firefox build tuned for privacy', 'Internet', 'ready'],
  ['signal-portable', 'Signal', 'Private messenger', 'Internet', 'ready'],
  ['qbittorrent-portable', 'qBittorrent', 'BitTorrent client', 'Internet', 'ready'],
  ['discord-portable', 'Discord', 'Voice and text chat', 'Internet', 'broken-manifest'],
];

/** PortableApps.com apps (`programs/portableapps.com/<App>Portable/`). */
const PORTABLE_APPS: readonly Seed[] = [
  ['FirefoxPortable', 'Mozilla Firefox', 'Web browser', 'Internet', 'ready'],
  ['7-ZipPortable', '7-Zip', 'File archiver', 'Utilities', 'ready'],
  ['VLCPortable', 'VLC Media Player', 'Media player', 'Music & Video', 'ready'],
  ['GIMPPortable', 'GIMP', 'Image editor', 'Graphics & Pictures', 'ready'],
  ['InkscapePortable', 'Inkscape', 'Vector graphics editor', 'Graphics & Pictures', 'ready'],
  ['KeePassXCPortable', 'KeePassXC', 'Password manager', 'Security', 'ready'],
  ['NotepadPlusPlusPortable', 'Notepad++', 'Text editor', 'Development', 'ready'],
  ['LibreOfficePortable', 'LibreOffice', 'Office suite', 'Office', 'ready'],
  ['AudacityPortable', 'Audacity', 'Audio editor and recorder', 'Music & Video', 'missing-exe'],
];

const FAVORITES = new Set(['explorer', 'terminal', 'vscodium-portable', 'FirefoxPortable']);

function entries(source: Source, seeds: readonly Seed[]): AppEntry[] {
  return seeds.map(([key, name, description, category, status, color]) => ({
    id: `${source}/${key}`,
    name,
    description,
    category,
    color: color ?? null,
    keywords: [],
    version: source === 'genslate' ? '0.1.0' : null,
    publisher: source === 'genslate' ? 'GENSLATE' : null,
    status,
    favorite: FAVORITES.has(key),
    hidden: false,
    args: [],
    hasIcon: source === 'genslate',
  }));
}

const THEME_PARAM = {
  name: 'mode',
  description: 'Theme',
  type: 'choice',
  values: ['system', 'dark', 'light'],
  required: true,
} as const;

/** The launcher's command registry, as `get_context` sends it (slash bar and tray share it). */
const ACTIONS: readonly ActionSpec[] = [
  {
    id: 'open',
    title: 'Open app',
    description: 'Launch an app',
    params: [{ name: 'app', description: 'App name or id', type: 'app', required: true }],
    effect: 'launch',
  },
  {
    id: 'theme',
    title: 'Change theme',
    description: 'Polar Night (dark), Snow Storm (light) or match the system',
    params: [THEME_PARAM],
    effect: 'writes-config',
  },
  {
    id: 'size',
    title: 'Change size',
    description: 'Small, medium or large window',
    params: [
      {
        name: 'preset',
        description: 'Size',
        type: 'choice',
        values: ['s', 'm', 'l'],
        required: true,
      },
    ],
    effect: 'writes-config',
  },
  {
    id: 'pin',
    title: 'Pin / unpin',
    description: 'Keep the launcher open when it loses focus',
    params: [],
    effect: 'window',
  },
  {
    id: 'settings',
    title: 'Settings',
    description: 'Open the Settings tool',
    params: [
      {
        name: 'section',
        description: 'Section',
        type: 'choice',
        values: ['appearance', 'behavior', 'keybindings', 'vault', 'about'],
        required: false,
      },
    ],
    effect: 'ui',
  },
  {
    id: 'vault',
    title: 'Vault',
    description: 'Unlock, lock or manage the encrypted vault',
    params: [],
    effect: 'ui',
  },
  {
    id: 'rescan',
    title: 'Rescan apps',
    description: 'Look for new or removed apps',
    params: [],
    effect: 'read',
  },
  { id: 'help', title: 'Help', description: 'Shortcuts and commands', params: [], effect: 'ui' },
  {
    id: 'ask',
    title: 'Ask AI',
    description: 'Ask the SLATECORE assistant (coming soon)',
    params: [{ name: 'prompt', description: 'Your question', type: 'text', required: false }],
    effect: 'ai',
  },
];

const TAB_LABEL: Record<Source, string> = {
  genslate: 'GENSLATE',
  portapps: 'portapps.io',
  portableapps: 'PortableApps.com',
};

/** Tab order everywhere: GENSLATE, portapps.io, PortableApps.com. */
const SOURCES: readonly Source[] = ['genslate', 'portapps', 'portableapps'];

function query(name: string): string | null {
  return new URLSearchParams(globalThis.location?.search ?? '').get(name);
}

/**
 * `?theme=snow-storm&size=l&pinned&status=usage&vault=uninitialized&empty&noThirdParty` in the
 * dev URL tweak the preview (handy for screenshots).
 */
export function mockOptionsFromUrl(): MockOptions {
  const theme = query('theme');
  const size = query('size');
  const vault = query('vault');
  return {
    preview: true,
    ...(query('empty') === null ? {} : { apps: [] }),
    noThirdParty: query('noThirdParty') !== null,
    pinned: query('pinned') !== null,
    status: query('status') === 'temps' ? 'temps' : 'usage',
    ...(theme === 'snow-storm' || theme === 'polar-night' ? { theme } : {}),
    ...(size === 's' || size === 'l' || size === 'm' ? { size } : {}),
    ...(vault === 'uninitialized' || vault === 'unlocked' ? { vault } : {}),
  };
}

/**
 * Where the browser preview anchors the tray menu: `?tray=top-right`, `bottom-left`,
 * `top-left`, or the default `bottom-right` (the Windows notification area).
 */
export function mockTrayAnchor(): TrayMenuAnchor {
  const corner = query('tray') ?? 'bottom-right';
  const opensUp = !corner.startsWith('top');
  const alignEnd = !corner.endsWith('left');
  const width = globalThis.innerWidth ?? 560;
  const height = globalThis.innerHeight ?? 440;
  return { x: alignEnd ? width - 12 : 12, y: opensUp ? height - 12 : 12, opensUp, alignEnd };
}

/** Crops an icon of the GENSLATE family to its plate, as the shell does. */
function iconDataUrl(icons: Readonly<Record<string, string>>, key: string): string {
  const svg = icons[key];
  if (svg === undefined) return '';
  const cropped = svg
    .replace('viewBox="0 0 1024 1024"', 'viewBox="100 100 824 824"')
    .replace(' width="1024" height="1024"', '');
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(cropped)}`;
}

function withAppearance(settings: Settings, change: Partial<Settings['config']['appearance']>) {
  return {
    ...settings,
    config: { ...settings.config, appearance: { ...settings.config.appearance, ...change } },
  };
}

const GB = 1024 ** 3;
const DAY = 86_400_000;
/** Wrong passwords before the vault starts asking you to wait. */
const FREE_ATTEMPTS = 5;
const THROTTLE_MS = 30_000;
const KDF = { mCostKib: 131_072, tCost: 3, pCost: 1 } as const;

function vaultError(code: VaultErrorCode, message: string, retryAfterMs?: number): VaultErrorDto {
  return retryAfterMs === undefined ? { code, message } : { code, message, retryAfterMs };
}

function seedVault(now: number): EntryDto[] {
  const file = (path: string, size: number, daysAgo: number): EntryDto => ({
    path,
    kind: 'file',
    size,
    modifiedMs: now - daysAgo * DAY,
  });
  const dir = (path: string, daysAgo: number): EntryDto => ({
    path,
    kind: 'dir',
    size: 0,
    modifiedMs: now - daysAgo * DAY,
  });
  return [
    dir('Contracts', 12),
    dir('Photos', 3),
    file('ideas.md', 4_812, 1),
    file('budget-2026.xlsx', 48_210, 6),
    file('Contracts/lease-2026.pdf', 412_337, 40),
    file('Contracts/insurance.pdf', 233_120, 75),
    file('Photos/aurora-01.jpg', 3_481_002, 3),
    file('Photos/aurora-02.jpg', 3_102_777, 3),
  ];
}

function parentOf(path: string): string {
  const slash = path.lastIndexOf('/');
  return slash === -1 ? '' : path.slice(0, slash);
}

function passwordProblem(password: string): VaultErrorDto | null {
  const length = [...password.normalize('NFKC')].length;
  return length < 8 || length > 1024
    ? vaultError('PASSWORD_REJECTED', 'Use 8 to 1024 characters.')
    : null;
}

export function createMockBackend(options: MockOptions = {}): MockBackend {
  const preview = options.preview === true;
  const icons = options.icons ?? {};
  let apps: AppEntry[] = options.apps
    ? [...options.apps]
    : [
        ...entries('genslate', GENSLATE),
        ...(options.noThirdParty ? [] : entries('portapps', PORTAPPS)),
        ...(options.noThirdParty ? [] : entries('portableapps', PORTABLE_APPS)),
      ];
  let recent: string[] = [
    ...(options.recent ?? [
      'genslate/editor',
      'portableapps/FirefoxPortable',
      'genslate/terminal',
      'portapps/brave-portable',
      'genslate/gallery',
    ]),
  ];
  let pinned = options.pinned === true;
  let settings: Settings = {
    config: {
      appearance: { theme: options.theme ?? 'system', size: options.size ?? 'm' },
      behavior: { hideOnBlur: true, hideOnLaunch: true, pinned },
      status: { mode: options.status ?? 'usage' },
    },
    keybindings: {
      global: { toggle: 'Ctrl+Alt+Space' },
      launcher: {
        focusSearch: 'mod+k',
        toggleTools: 'mod+,',
        togglePin: 'mod+p',
        toggleFavorite: 'mod+d',
        tabGenslate: 'mod+1',
        tabPortapps: 'mod+2',
        tabPortableapps: 'mod+3',
      },
    },
    issue: null,
  };

  type Handler = (payload: unknown) => void;
  const listeners = new Map<keyof LauncherEvents, Set<Handler>>();
  const emit = <E extends keyof LauncherEvents>(event: E, payload: LauncherEvents[E]) => {
    for (const handler of listeners.get(event) ?? []) handler(payload);
  };
  const later = (ms: number) =>
    preview ? new Promise<void>((resolve) => setTimeout(resolve, ms)) : Promise.resolve();

  let telemetryTimer: ReturnType<typeof setInterval> | undefined;
  let tick = 0;
  const sample = (): Telemetry => {
    tick += 1;
    const wave = (base: number, amplitude: number, speed: number) =>
      Math.round((base + Math.sin(tick / speed) * amplitude) * 10) / 10;
    return {
      cpuTempC: wave(47, 4, 3),
      gpuTempC: wave(51, 3, 4),
      cpuUsagePct: wave(14, 9, 2),
      gpuUsagePct: wave(23, 10, 5),
      netDownBps: Math.round(wave(1_200_000, 700_000, 2)),
      netUpBps: Math.round(wave(82_000, 40_000, 3)),
    };
  };

  const tabs = (): TabInfo[] =>
    SOURCES.map((source) => ({
      source,
      label: TAB_LABEL[source],
      count: apps.filter((app) => app.id.startsWith(`${source}/`) && !app.hidden).length,
    })).filter((tab) => tab.source === 'genslate' || tab.count > 0);

  const update = (id: string, change: (app: AppEntry) => AppEntry) => {
    apps = apps.map((app) => (app.id === id ? change(app) : app));
    emit('catalog', undefined);
  };

  // ── Vault ────────────────────────────────────────────────────────────────
  const initial = options.vault ?? 'locked';
  let vaultState: VaultState = initial;
  let vaultPassword: string | null = initial === 'uninitialized' ? null : MOCK_VAULT_PASSWORD;
  let vaultEntries: EntryDto[] = initial === 'uninitialized' ? [] : seedVault(Date.now());
  let failedAttempts = 0;
  let throttledUntil = 0;
  let sessionFiles = 0;

  const vaultStatus = (): VaultStatusDto => {
    const wait = throttledUntil - Date.now();
    return {
      state: vaultState,
      kdf: vaultState === 'uninitialized' ? null : KDF,
      kdfUpgradePending: false,
      failedAttempts,
      retryAfterMs: wait > 0 ? wait : null,
      foreignItems: 0,
      entryCount:
        vaultState === 'unlocked'
          ? vaultEntries.filter((entry) => entry.kind === 'file').length
          : null,
      sessionFiles,
    };
  };
  const vaultChanged = () => {
    const status = vaultStatus();
    emit('vault', status);
    return status;
  };
  const requireUnlocked = () => {
    if (vaultState === 'uninitialized') throw vaultError('UNINITIALIZED', 'There is no vault yet.');
    if (vaultState === 'locked') throw vaultError('LOCKED', 'The vault is locked.');
  };
  const checkPassword = (password: string) => {
    const wait = throttledUntil - Date.now();
    if (wait > 0) {
      throw vaultError('THROTTLED', 'Too many wrong passwords.', wait);
    }
    if (password !== vaultPassword) {
      failedAttempts += 1;
      if (failedAttempts >= FREE_ATTEMPTS) throttledUntil = Date.now() + THROTTLE_MS;
      vaultChanged();
      throw vaultError('WRONG_PASSWORD', 'Wrong password.');
    }
    failedAttempts = 0;
    throttledUntil = 0;
  };

  const context: LauncherContext = {
    version: '0.1.0',
    mode: 'web',
    suiteName: 'SLATECORE',
    profile: 'Shared',
    settings,
    pinned,
    actions: ACTIONS,
    layout: { inset: 16, normalWidth: 460, expandedWidth: 920 },
  };

  return {
    emit,
    context: async () => ({ ...context, settings, pinned }),
    listApps: async () => ({ apps, tabs: tabs(), recent }),
    rescan: async () => {
      await later(600);
      emit('catalog', undefined);
    },
    launch: async (id) => {
      recent = [id, ...recent.filter((other) => other !== id)].slice(0, 8);
      update(id, (app) => ({ ...app, status: 'running' }));
    },
    openAppFolder: async () => {},
    setOverride: async (id, patch) =>
      update(id, (app) => ({
        ...app,
        ...(patch.favorite === undefined ? {} : { favorite: patch.favorite }),
        ...(patch.hidden === undefined ? {} : { hidden: patch.hidden }),
        ...(patch.name === undefined || patch.name === null ? {} : { name: patch.name }),
      })),
    openFolder: async () => {},
    openConfigFile: async () => {},
    setSetting: async (key, value) => {
      if (
        key === 'theme' &&
        (value === 'system' || value === 'polar-night' || value === 'snow-storm')
      )
        settings = withAppearance(settings, { theme: value });
      else if (key === 'size' && (value === 's' || value === 'm' || value === 'l'))
        settings = withAppearance(settings, { size: value });
      else if (key === 'statusMode' && (value === 'temps' || value === 'usage'))
        settings = { ...settings, config: { ...settings.config, status: { mode: value } } };
      else return;
      emit('settings', settings);
    },
    volume: async () => ({
      label: 'E:',
      name: 'SLATECORE',
      totalBytes: 476.9 * GB,
      availableBytes: 312.4 * GB,
      removable: true,
    }),
    setTelemetryActive: async (active) => {
      if (telemetryTimer !== undefined) clearInterval(telemetryTimer);
      // Readings drift once a second in the preview; tests get one sample.
      telemetryTimer =
        active && preview ? setInterval(() => emit('telemetry', sample()), 1000) : undefined;
      if (active) emit('telemetry', sample());
    },
    setPinned: async (next) => {
      pinned = next;
      settings = {
        ...settings,
        config: { ...settings.config, behavior: { ...settings.config.behavior, pinned: next } },
      };
      emit('pinned', next);
    },
    setExpanded: async () => {},
    setPopupOpen: async () => {},
    hide: async () => {
      emit('willHide', undefined);
      // A browser tab can't hide: come back so the page stays usable.
      if (preview) setTimeout(() => emit('shown', 'apps'), 700);
    },
    show: async (view) => emit('shown', view ?? 'apps'),
    // The browser preview of the tray menu reopens it, so the page stays usable.
    hideTrayMenu: async () => {
      if (preview) setTimeout(() => emit('trayMenuOpen', mockTrayAnchor()), 700);
    },
    quit: async () => {},
    runAction: async (id) => ({ kind: 'done', message: `/${id} runs in the desktop app` }),
    on: async (event, handler) => {
      const set = listeners.get(event) ?? new Set<Handler>();
      const wrapped: Handler = (payload) => handler(payload as never);
      set.add(wrapped);
      listeners.set(event, set);
      return () => set.delete(wrapped);
    },
    iconUrl: (id) => {
      const [source, key] = id.split('/');
      return source === 'genslate' && key !== undefined ? iconDataUrl(icons, key) : '';
    },

    vaultStatus: async () => vaultStatus(),
    vaultCreate: async (password) => {
      if (vaultState !== 'uninitialized') throw vaultError('ALREADY_EXISTS', 'A vault exists.');
      const problem = passwordProblem(password);
      if (problem !== null) throw problem;
      await later(700);
      vaultPassword = password;
      vaultEntries = [];
      vaultState = 'unlocked';
      return vaultChanged();
    },
    vaultUnlock: async (password) => {
      if (vaultState === 'uninitialized')
        throw vaultError('UNINITIALIZED', 'There is no vault yet.');
      if (vaultState === 'unlocked') throw vaultError('ALREADY_UNLOCKED', 'Already unlocked.');
      await later(700);
      checkPassword(password);
      vaultState = 'unlocked';
      return vaultChanged();
    },
    vaultLock: async () => {
      requireUnlocked();
      const wipedFiles = sessionFiles;
      sessionFiles = 0;
      vaultState = 'locked';
      vaultChanged();
      return { synced: [], unsynced: [], wipedFiles, undeletable: [] };
    },
    vaultChangePassword: async (current, next) => {
      requireUnlocked();
      const problem = passwordProblem(next);
      if (problem !== null) throw problem;
      await later(900);
      checkPassword(current);
      vaultPassword = next;
      return vaultChanged();
    },
    vaultList: async (dir) => {
      requireUnlocked();
      return vaultEntries
        .filter((entry) => parentOf(entry.path) === dir)
        .toSorted(
          (a, b) =>
            (a.kind === b.kind ? 0 : a.kind === 'dir' ? -1 : 1) || a.path.localeCompare(b.path),
        );
    },
    vaultImport: async (sources, destDir) => {
      requireUnlocked();
      const added = sources.map((source) => {
        const name = source.split(/[\\/]/).at(-1) ?? source;
        return {
          path: destDir === '' ? name : `${destDir}/${name}`,
          kind: 'file' as const,
          size: 1024,
          modifiedMs: Date.now(),
        };
      });
      vaultEntries = [...vaultEntries, ...added];
      vaultChanged();
      return added;
    },
    vaultExport: async () => {
      requireUnlocked();
    },
    vaultOpen: async (path) => {
      requireUnlocked();
      if (!vaultEntries.some((entry) => entry.path === path && entry.kind === 'file'))
        throw vaultError('NOT_FOUND', 'That file is not in the vault.');
      await later(250);
      sessionFiles += 1;
      vaultChanged();
      return { sessionPath: `other/launcher/cache/vault-session/${path}` };
    },
    vaultVerify: async () => {
      requireUnlocked();
      await later(500);
      return {
        filesChecked: vaultEntries.filter((entry) => entry.kind === 'file').length,
        orphanBlobs: 0,
        problems: [],
      };
    },
  };
}

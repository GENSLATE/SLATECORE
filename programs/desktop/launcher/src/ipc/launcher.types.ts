/**
 * Types of everything the Rust side sends (`genslate-launcher-core`, `genslate-vault` and
 * `src-tauri/src/commands.rs`). Keep in step with the Rust structs; `launcher.parse.ts` checks
 * payloads at the boundary.
 */

export type Source = 'genslate' | 'portableapps' | 'portapps';

export type AppStatus = 'ready' | 'running' | 'not-installed' | 'missing-exe' | 'broken-manifest';

export interface AppEntry {
  /** `<source>/<key>`, e.g. `genslate/explorer`. */
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly category: string;
  /** Nord token of the icon tile (`nord9`), GENSLATE apps only. */
  readonly color: string | null;
  readonly keywords: readonly string[];
  readonly version: string | null;
  readonly publisher: string | null;
  readonly status: AppStatus;
  readonly favorite: boolean;
  readonly hidden: boolean;
  readonly args: readonly string[];
  readonly hasIcon: boolean;
}

export interface TabInfo {
  readonly source: Source;
  readonly label: string;
  readonly count: number;
}

export interface AppList {
  readonly apps: readonly AppEntry[];
  readonly tabs: readonly TabInfo[];
  /** Recently launched ids, newest first. */
  readonly recent: readonly string[];
}

export type ThemeSetting = 'system' | 'polar-night' | 'snow-storm';
export type SizePreset = 's' | 'm' | 'l';
export type StatusMode = 'temps' | 'usage';

export interface LauncherConfig {
  readonly appearance: { readonly theme: ThemeSetting; readonly size: SizePreset };
  readonly behavior: {
    readonly hideOnBlur: boolean;
    readonly hideOnLaunch: boolean;
    readonly pinned: boolean;
  };
  readonly status: { readonly mode: StatusMode };
}

export interface LauncherKeys {
  readonly focusSearch: string;
  /** Opens or closes the Settings tool. */
  readonly toggleTools: string;
  readonly togglePin: string;
  readonly toggleFavorite: string;
  readonly tabGenslate: string;
  readonly tabPortableapps: string;
  readonly tabPortapps: string;
}

export interface Keybindings {
  readonly global: { readonly toggle: string };
  readonly launcher: LauncherKeys;
}

export interface Settings {
  readonly config: LauncherConfig;
  readonly keybindings: Keybindings;
  /** Why a settings file was ignored (the previous values stay in use). */
  readonly issue: string | null;
}

export type Effect = 'ui' | 'window' | 'read' | 'launch' | 'open' | 'writes-config' | 'ai';

export type ParamSpec = {
  readonly name: string;
  readonly description: string;
  readonly required: boolean;
} & (
  | { readonly type: 'app' }
  | { readonly type: 'choice'; readonly values: readonly string[] }
  | { readonly type: 'text' }
);

export interface ActionSpec {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly params: readonly ParamSpec[];
  readonly effect: Effect;
}

/** Where the launcher runs from (`genslate-paths` modes, plus the browser preview). */
export type RunMode = 'suite' | 'dev' | 'web';

export interface FrameLayout {
  /** Transparent border around the frame (logical px). */
  readonly inset: number;
  /** Frame width with the app list (logical px). */
  readonly normalWidth: number;
  /** Frame width while a tool is open (logical px). */
  readonly expandedWidth: number;
}

export interface LauncherContext {
  readonly version: string;
  readonly mode: RunMode;
  /** The install folder's name (shown as the suite name). */
  readonly suiteName: string | null;
  readonly profile: string;
  readonly settings: Settings;
  readonly pinned: boolean;
  readonly actions: readonly ActionSpec[];
  readonly layout: FrameLayout;
}

export interface VolumeInfo {
  /** Drive letter, e.g. `E:`. */
  readonly label: string;
  /** Volume name, e.g. `SLATECORE`. */
  readonly name: string | null;
  readonly totalBytes: number;
  readonly availableBytes: number;
  readonly removable: boolean;
}

/** `null` = not available on this machine (hidden). */
export interface Telemetry {
  readonly cpuTempC: number | null;
  readonly gpuTempC: number | null;
  readonly cpuUsagePct: number | null;
  readonly gpuUsagePct: number | null;
  readonly netDownBps: number | null;
  readonly netUpBps: number | null;
}

export type SharedFolder =
  | 'desktop'
  | 'documents'
  | 'downloads'
  | 'music'
  | 'pictures'
  | 'videos'
  | 'storage';

export type ConfigFile = 'settings' | 'keybindings' | 'logs';

export type SettingKey = 'theme' | 'size' | 'statusMode';

/** The view the launcher opens on when the shell shows it (the tray menu can ask for one). */
export type ShowView = 'apps' | 'help' | 'settings';

/** Where the tray menu window anchors its menu (logical px from the window's top-left). */
export interface TrayMenuAnchor {
  readonly x: number;
  readonly y: number;
  /** Open upwards (tray at the bottom of the screen). */
  readonly opensUp: boolean;
  /** Right-align the menu on the anchor (tray on the right). */
  readonly alignEnd: boolean;
}

/** Absent = keep, `null` = remove. */
export interface OverridePatch {
  readonly favorite?: boolean;
  readonly hidden?: boolean;
  readonly name?: string | null;
  readonly category?: string | null;
  readonly args?: readonly string[] | null;
}

export type ActionOutcome =
  | { readonly kind: 'done'; readonly message: string | null }
  | { readonly kind: 'ui'; readonly id: string };

// ── Vault (`genslate-vault`, research/vault.md §8.3) ──────────────────────────

/** The Tauri commands of the vault, by their final names. */
export const VAULT_COMMANDS = {
  status: 'vault_status',
  create: 'vault_create',
  unlock: 'vault_unlock',
  lock: 'vault_lock',
  changePassword: 'vault_change_password',
  list: 'vault_list',
  import: 'vault_import',
  export: 'vault_export',
  open: 'vault_open',
  verify: 'vault_verify',
} as const;

export type VaultState = 'uninitialized' | 'locked' | 'unlocked';

export interface KdfParamsDto {
  readonly mCostKib: number;
  readonly tCost: number;
  readonly pCost: number;
}

export interface VaultStatusDto {
  readonly state: VaultState;
  readonly kdf: KdfParamsDto | null;
  readonly kdfUpgradePending: boolean;
  readonly failedAttempts: number;
  readonly retryAfterMs: number | null;
  /** Plain files dropped into `storage/vault`, encrypted at the next unlock. */
  readonly foreignItems: number;
  /** Files in the vault (known only while unlocked). */
  readonly entryCount: number | null;
  /** Decrypted files currently in the session folder. */
  readonly sessionFiles: number;
}

export interface EntryDto {
  /** Vault path with `/` separators (`Notes/ideas.md`). */
  readonly path: string;
  readonly kind: 'file' | 'dir';
  readonly size: number;
  readonly modifiedMs: number;
}

export interface ProgressDto {
  readonly done: number;
  readonly total: number;
}

export interface LockReportDto {
  readonly synced: readonly string[];
  readonly unsynced: readonly string[];
  readonly wipedFiles: number;
  readonly undeletable: readonly string[];
}

export type VaultProblem = 'blobMissing' | 'tampered' | 'truncated' | 'sizeMismatch';

export interface VerifyReportDto {
  readonly filesChecked: number;
  readonly orphanBlobs: number;
  readonly problems: readonly { readonly path: string; readonly problem: VaultProblem }[];
}

export type VaultConflict = 'fail' | 'replace' | 'keepBoth';

export type VaultErrorCode =
  | 'UNINITIALIZED'
  | 'ALREADY_EXISTS'
  | 'LOCKED'
  | 'ALREADY_UNLOCKED'
  | 'WRONG_PASSWORD'
  | 'THROTTLED'
  | 'PASSWORD_REJECTED'
  | 'BUSY'
  | 'HEADER_DAMAGED'
  | 'UNSUPPORTED_VERSION'
  | 'UNSUPPORTED_KDF'
  | 'OUT_OF_MEMORY'
  | 'TAMPERED'
  | 'BLOB_MISSING'
  | 'NOT_FOUND'
  | 'EXISTS'
  | 'INVALID_PATH'
  | 'LIMIT_EXCEEDED'
  | 'FILE_TOO_LARGE'
  | 'CANCELLED'
  | 'UNSYNCED_EDITS'
  | 'IO'
  | 'INTERNAL';

export interface VaultErrorDto {
  readonly code: VaultErrorCode;
  readonly message: string;
  readonly retryAfterMs?: number;
  readonly paths?: readonly string[];
}

/** Events from the shell and their payloads. */
export interface LauncherEvents {
  readonly shown: ShowView;
  readonly willHide: undefined;
  readonly pinned: boolean;
  readonly settings: Settings;
  readonly catalog: undefined;
  readonly telemetry: Telemetry;
  /** `vault://status`: every vault state change, automatic locks included. */
  readonly vault: VaultStatusDto;
  /** Tray menu window only: open at this anchor. */
  readonly trayMenuOpen: TrayMenuAnchor;
  /** Tray menu window only: focus left, close. */
  readonly trayMenuClose: undefined;
}

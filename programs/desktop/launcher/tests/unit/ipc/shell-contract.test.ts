/**
 * The IPC contract with the Rust shell, checked against the shell's source
 * (`src-tauri/src/{commands,vault_commands,actions}.rs`, `lib.rs`, `build.rs` and the window
 * capabilities): every command the UI invokes exists, is registered and permitted for the
 * window that calls it, and gets exactly the argument names Rust declares (Tauri maps a
 * snake_case Rust parameter to a camelCase key), with enum values Rust accepts.
 */
import { afterEach, describe, expect, test } from 'bun:test';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { loadBoot, loadTrayBoot } from '../../../src/app/launcher.provider';
import { runCommand } from '../../../src/app/launcher-commands.model';
import { trayCommandHost } from '../../../src/features/tray-menu/use-tray-commands.hook';
import {
  createBackend,
  LAUNCHER_EVENT_NAMES,
  type LauncherBackend,
} from '../../../src/ipc/launcher.client';
import { createMockBackend } from '../../../src/ipc/launcher.mock';
import type {
  ConfigFile,
  SettingKey,
  SharedFolder,
  ShowView,
  VaultConflict,
} from '../../../src/ipc/launcher.types';

const SRC_TAURI = new URL('../../../src-tauri/', import.meta.url);
const read = (path: string) => Bun.file(new URL(path, SRC_TAURI)).text();

interface RustParam {
  readonly name: string;
  readonly type: string;
}

const snakeToCamel = (name: string) =>
  name.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());

/**
 * Parameter types Tauri fills in itself (the caller's window or webview, the app, managed state,
 * the raw request or a scope); the webview sends no argument for them.
 */
const INJECTED =
  /^(?:tauri::)?(?:ipc::)?(?:AppHandle|State|Window|WebviewWindow|Webview|Request|CommandScope|GlobalScope)\b/;

/** `#[tauri::command]` functions and their webview-supplied parameters. */
async function rustCommands(): Promise<Map<string, readonly RustParam[]>> {
  const commands = new Map<string, readonly RustParam[]>();
  for (const file of ['src/commands.rs', 'src/vault_commands.rs', 'src/actions.rs']) {
    const source = await read(file);
    for (const match of source.matchAll(
      /#\[tauri::command\]\s*pub (?:async )?fn (\w+)\(([\s\S]*?)\)\s*(?:->|\{)/g,
    )) {
      const params = (match[2] ?? '')
        .split(/,\s*(?=\w+:)/)
        .map((part) => part.trim().replace(/,$/, ''))
        .filter((part) => part !== '')
        .map((part): RustParam => {
          const [name = '', ...type] = part.split(':');
          return { name: name.trim(), type: type.join(':').trim() };
        })
        .filter((param) => !INJECTED.test(param.type));
      commands.set(match[1] ?? '', params);
    }
  }
  return commands;
}

/** Commands in `generate_handler!` and in build.rs's permission list. */
async function registered(): Promise<{ handler: string[]; build: string[] }> {
  const lib = await read('src/lib.rs');
  const block = /generate_handler!\[([\s\S]*?)\]/.exec(lib)?.[1] ?? '';
  const build = /const COMMANDS: &\[&str\] = &\[([\s\S]*?)\];/.exec(await read('build.rs'))?.[1];
  return {
    handler: [...block.matchAll(/\w+::(\w+)/g)].map((match) => match[1] ?? ''),
    build: [...(build ?? '').matchAll(/"(\w+)"/g)].map((match) => match[1] ?? ''),
  };
}

/** The commands a window's capability allows (`allow-open-folder` → `open_folder`). */
async function allowed(capability: 'main' | 'tray-menu'): Promise<Set<string>> {
  const json: unknown = JSON.parse(await read(`capabilities/${capability}.capability.json`));
  const permissions: unknown =
    typeof json === 'object' && json !== null ? Reflect.get(json, 'permissions') : undefined;
  if (!Array.isArray(permissions)) throw new Error(`${capability}: no permissions`);
  return new Set(
    permissions.flatMap((permission) =>
      typeof permission === 'string' && permission.startsWith('allow-')
        ? [permission.slice('allow-'.length).replaceAll('-', '_')]
        : [],
    ),
  );
}

/** The serialised variants of a Rust enum (its `rename_all` applied). */
async function rustEnum(file: string, name: string): Promise<string[]> {
  const source = await read(file);
  const match = new RegExp(
    `#\\[serde\\(rename_all = "([\\w-]+)"\\)\\]\\s*pub enum ${name} \\{([\\s\\S]*?)\\n\\}`,
  ).exec(source);
  if (match === null) throw new Error(`enum ${name} not found in ${file}`);
  const [, rule, body = ''] = match;
  const variants = body
    .split('\n')
    .map((line) => /^\s{4}([A-Z]\w*),?$/.exec(line)?.[1])
    .filter((variant) => variant !== undefined);
  const words = (variant: string) => variant.split(/(?=[A-Z])/).map((word) => word.toLowerCase());
  return variants.map((variant) => {
    if (rule === 'lowercase') return variant.toLowerCase();
    if (rule === 'kebab-case') return words(variant).join('-');
    if (rule === 'camelCase') return variant.charAt(0).toLowerCase() + variant.slice(1);
    throw new Error(`rename_all ${rule} not handled`);
  });
}

// Every value of the UI's unions (the `satisfies` makes a missing or extra one a type error).
const FOLDERS = every<SharedFolder>({
  desktop: 0,
  documents: 0,
  downloads: 0,
  music: 0,
  pictures: 0,
  videos: 0,
  storage: 0,
});
const CONFIG_FILES = every<ConfigFile>({ settings: 0, keybindings: 0, logs: 0 });
const SETTING_KEYS = every<SettingKey>({ theme: 0, size: 0, statusMode: 0 });
const SHOW_VIEWS = every<ShowView>({ apps: 0, help: 0, settings: 0 });
const CONFLICTS = every<VaultConflict>({ fail: 0, replace: 0, keepBoth: 0 });

type Calls = {
  readonly [K in Exclude<keyof LauncherBackend, 'on' | 'iconUrl'>]: (
    backend: LauncherBackend,
  ) => Promise<unknown>[];
};

/** Calls every backend method, with every enum value it can send. */
const CALLS: Calls = {
  context: (b) => [b.context()],
  listApps: (b) => [b.listApps()],
  rescan: (b) => [b.rescan()],
  launch: (b) => [b.launch('genslate/terminal'), b.launch('genslate/terminal', ['--new'])],
  openAppFolder: (b) => [b.openAppFolder('genslate/terminal')],
  setOverride: (b) => [b.setOverride('genslate/terminal', { favorite: true })],
  openFolder: (b) => FOLDERS.map((folder) => b.openFolder(folder)),
  openConfigFile: (b) => CONFIG_FILES.map((file) => b.openConfigFile(file)),
  setSetting: (b) => SETTING_KEYS.map((key) => b.setSetting(key, 'x')),
  volume: (b) => [b.volume()],
  setTelemetryActive: (b) => [b.setTelemetryActive(true)],
  setPinned: (b) => [b.setPinned(true)],
  setExpanded: (b) => [b.setExpanded(true)],
  setPopupOpen: (b) => [b.setPopupOpen(true)],
  hide: (b) => [b.hide()],
  show: (b) => [b.show(), ...SHOW_VIEWS.map((view) => b.show(view))],
  hideTrayMenu: (b) => [b.hideTrayMenu()],
  quit: (b) => [b.quit()],
  runAction: (b) => [b.runAction('fav', { app: 'genslate/terminal' })],
  vaultStatus: (b) => [b.vaultStatus()],
  vaultCreate: (b) => [b.vaultCreate('polar-night-77')],
  vaultUnlock: (b) => [b.vaultUnlock('polar-night-77')],
  vaultLock: (b) => [b.vaultLock(), b.vaultLock(true)],
  vaultChangePassword: (b) => [b.vaultChangePassword('polar-night-77', 'snow-storm-88')],
  vaultList: (b) => [b.vaultList('')],
  vaultImport: (b) => CONFLICTS.map((conflict) => b.vaultImport(['a.txt'], '', conflict)),
  vaultExport: (b) => [b.vaultExport('ideas.md', 'ideas.md')],
  vaultOpen: (b) => [b.vaultOpen('ideas.md')],
  vaultVerify: (b) => [b.vaultVerify()],
};

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** The keys of an exhaustive `Record<Union, 0>`: every value of the union, typed. */
function every<T extends string>(record: Record<T, 0>): T[] {
  return Object.keys(record).filter((key): key is T => key in record);
}

interface Invocation {
  readonly command: string;
  readonly args: Readonly<Record<string, unknown>>;
}

/** Well-formed replies, taken from the browser mock (its payloads are the shell's shapes). */
async function replies(): Promise<Record<string, unknown>> {
  const mock = createMockBackend({ vault: 'unlocked' });
  return {
    get_context: await mock.context(),
    list_apps: await mock.listApps(),
    get_volume_info: await mock.volume(),
    run_action: { kind: 'done', message: null },
    vault_status: await mock.vaultStatus(),
    vault_create: await mock.vaultStatus(),
    vault_unlock: await mock.vaultStatus(),
    vault_change_password: await mock.vaultStatus(),
    vault_lock: { synced: [], unsynced: [], wipedFiles: 0, undeletable: [] },
    vault_list: [],
    vault_import: [],
    vault_open: { sessionPath: 'ideas.md' },
    vault_verify: { filesChecked: 0, orphanBlobs: 0, problems: [] },
  };
}

/** Runs `work` against the Tauri backend and returns every command it invoked. */
async function recording(work: (backend: LauncherBackend) => Promise<unknown>) {
  const answers = await replies();
  const invoked: Invocation[] = [];
  Reflect.set(globalThis, 'isTauri', true);
  mockIPC((command, args) => {
    invoked.push({ command, args: isRecord(args) ? args : {} });
    return answers[command] ?? null;
  });
  await work(await createBackend());
  return invoked;
}

afterEach(() => {
  clearMocks();
  Reflect.deleteProperty(globalThis, 'isTauri');
});

describe('the shell contract', () => {
  test('every command the UI invokes exists, is registered and has a permission', async () => {
    const rust = await rustCommands();
    const { handler, build } = await registered();
    const invoked = await recording((backend) =>
      Promise.all(Object.values(CALLS).flatMap((call) => call(backend))),
    );
    const names = [...new Set(invoked.map((call) => call.command))].sort();
    expect(names.length).toBe(29);
    for (const name of names) {
      expect({
        name,
        rust: rust.has(name),
        handler: handler.includes(name),
        build: build.includes(name),
      }).toEqual({
        name,
        rust: true,
        handler: true,
        build: true,
      });
    }
  });

  test('argument keys are the camelCase names of the Rust parameters', async () => {
    const rust = await rustCommands();
    // Injected parameters are skipped, nothing else: the caller's window is Tauri's to fill.
    expect(rust.get('launch_app')?.map((param) => param.name)).toEqual(['id', 'args']);
    expect(rust.get('vault_import')?.map((param) => param.name)).toEqual([
      'sources',
      'dest_dir',
      'on_conflict',
    ]);
    const invoked = await recording((backend) =>
      Promise.all(Object.values(CALLS).flatMap((call) => call(backend))),
    );
    for (const { command, args } of invoked) {
      const params = rust.get(command) ?? [];
      const keys = Object.keys(args).sort();
      const required = params.filter((param) => !param.type.startsWith('Option<'));
      expect({
        command,
        unknown: keys.filter((key) => !params.some((p) => snakeToCamel(p.name) === key)),
      }).toEqual({
        command,
        unknown: [],
      });
      expect({
        command,
        missing: required.map((p) => snakeToCamel(p.name)).filter((key) => !keys.includes(key)),
      }).toEqual({
        command,
        missing: [],
      });
    }
  });

  test('enum arguments use values the Rust enums accept', async () => {
    const accepted: Record<string, Promise<string[]>> = {
      folder: rustEnum('src/folders.rs', 'SharedFolder'),
      file: rustEnum('src/folders.rs', 'ConfigFile'),
      key: rustEnum('src/settings_write.rs', 'SettingKey'),
      view: rustEnum('src/window.rs', 'ShowView'),
      onConflict: rustEnum('src/vault_dto.rs', 'ConflictDto'),
    };
    const invoked = await recording((backend) =>
      Promise.all(Object.values(CALLS).flatMap((call) => call(backend))),
    );
    const checked = new Set<string>();
    for (const { command, args } of invoked) {
      for (const [key, values] of Object.entries(accepted)) {
        if (!(key in args) || command === 'run_action') continue;
        checked.add(key);
        expect({
          command,
          key,
          value: args[key],
          ok: (await values).includes(String(args[key])),
        }).toEqual({
          command,
          key,
          value: args[key],
          ok: true,
        });
      }
    }
    expect([...checked].sort()).toEqual(Object.keys(accepted).sort());
  });

  test('the launcher window only calls commands its capability allows', async () => {
    const main = await allowed('main');
    const invoked = await recording(async (backend) => {
      await loadBoot(backend);
      await Promise.all(
        Object.entries(CALLS)
          .filter(([method]) => method !== 'hideTrayMenu')
          .flatMap(([, call]) => call(backend)),
      );
    });
    for (const { command } of invoked) {
      expect({ command, allowed: main.has(command) }).toEqual({ command, allowed: true });
    }
  });

  test('the tray menu window boots and runs every row with only its own permissions', async () => {
    const tray = await allowed('tray-menu');
    const sources = await Promise.all(
      ['tray-menu.component.tsx', 'tray-menu-submenus.component.tsx'].map((file) =>
        Bun.file(new URL(`../../../src/features/tray-menu/${file}`, import.meta.url)).text(),
      ),
    );
    const rows = [...sources.join('\n').matchAll(/run\('(\w+)'/g)].map((match) => match[1] ?? '');
    expect(rows).toEqual(expect.arrayContaining(['show', 'folder', 'config', 'keys', 'logs']));
    const params: Readonly<Record<string, Record<string, string>>> = {
      open: { app: 'genslate/terminal' },
      folder: { name: 'documents' },
      theme: { mode: 'snow-storm' },
      size: { preset: 'l' },
    };
    const invoked = await recording(async (backend) => {
      const boot = await loadTrayBoot(backend);
      const host = trayCommandHost(backend, boot.list.apps, boot.context.pinned);
      for (const row of rows) runCommand(host, row, params[row] ?? {});
      await backend.hideTrayMenu();
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    const commands = [...new Set(invoked.map((call) => call.command))];
    expect(commands).toEqual(expect.arrayContaining(['get_context', 'list_apps', 'window_show']));
    for (const command of commands) {
      expect({ command, allowed: tray.has(command) }).toEqual({ command, allowed: true });
    }
  });

  test('the [vault] timers arrive under the names the Rust config serialises', async () => {
    const config = await Bun.file(
      new URL('../../../../../../crates/launcher-core/src/config.rs', import.meta.url),
    ).text();
    const match =
      /rename_all\(serialize = "camelCase", deserialize = "kebab-case"\)\s*\)\]\s*pub struct VaultSettings \{([\s\S]*?)\n\}/.exec(
        config,
      );
    const fields = [...(match?.[1] ?? '').matchAll(/pub (\w+): u32,/g)].map((m) =>
      snakeToCamel(m[1] ?? ''),
    );
    expect(fields).toEqual(['idleLockMinutes', 'hideLockMinutes']);
    const { settings } = await createMockBackend().context();
    expect(Object.keys(settings.config.vault)).toEqual(fields);
    expect(/pub vault: VaultSettings,/.test(config)).toBe(true);
  });

  test('the UI listens to exactly the events the shell emits, by name', async () => {
    const source = await read('src/events.rs');
    const rust = [...source.matchAll(/pub const \w+: &str = "([^"]+)";/g)].map((m) => m[1] ?? '');
    expect(rust.length).toBeGreaterThan(9);
    expect(Object.values(LAUNCHER_EVENT_NAMES).sort()).toEqual(rust.sort());
  });
});

import { describe, expect, test } from 'bun:test';

import {
  COMMAND_IDS,
  type CommandHost,
  type CommandTarget,
  runCommand,
} from '../../../src/app/launcher-commands.model';
import { createMockBackend } from '../../../src/ipc/launcher.mock';
import type { AppEntry } from '../../../src/ipc/launcher.types';
import { app } from '../fixtures';
import { spyBackend } from '../launcher.harness';

const APPS = [app('genslate/terminal'), app('genslate/explorer', { name: 'Explorer' })];

function host(overrides: Partial<CommandHost> = {}) {
  const backend = spyBackend(createMockBackend());
  const launched: AppEntry[] = [];
  const shown: CommandTarget[] = [];
  const told: [string, string][] = [];
  const failed: unknown[] = [];
  const value: CommandHost = {
    backend,
    apps: APPS,
    pinned: false,
    launch: (entry) => launched.push(entry),
    show: (target) => shown.push(target),
    inform: (message, tone) => told.push([message, tone]),
    fail: (error) => failed.push(error),
    ...overrides,
  };
  return { host: value, backend, launched, shown, told, failed };
}

/** Lets the handlers' backend promises settle. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('launcher commands', () => {
  test('every command of the shell registry has a handler of its own', async () => {
    const { actions } = await createMockBackend().context();
    const known: readonly string[] = COMMAND_IDS;
    for (const action of actions) expect(known).toContain(action.id);
  });

  test('open launches by id or by the best name match, and says when nothing matches', () => {
    const { host: h, launched, told } = host();
    runCommand(h, 'open', { app: 'genslate/explorer' });
    runCommand(h, 'open', { app: 'term' });
    runCommand(h, 'open', { app: 'zzz' });
    expect(launched.map((entry) => entry.id)).toEqual(['genslate/explorer', 'genslate/terminal']);
    expect(told).toEqual([['No app matches “zzz”', 'info']]);
  });

  test('theme and size write settings from slash words or theme names', () => {
    const { host: h, backend } = host();
    runCommand(h, 'theme', { mode: 'dark' });
    runCommand(h, 'theme', { mode: 'snow-storm' });
    runCommand(h, 'theme', { mode: 'neon' });
    runCommand(h, 'size', { preset: 'l' });
    runCommand(h, 'size', { preset: 'xl' });
    expect(backend.setSetting.mock.calls).toEqual([
      ['theme', 'polar-night'],
      ['theme', 'snow-storm'],
      ['size', 'l'],
    ]);
  });

  test('views: settings (with a section), vault, help, ask and show', () => {
    const { host: h, shown } = host();
    runCommand(h, 'settings', { section: 'keybindings' });
    runCommand(h, 'settings', {});
    runCommand(h, 'settings', { section: 'nowhere' });
    runCommand(h, 'vault');
    runCommand(h, 'help');
    runCommand(h, 'ask', { prompt: 'hi' });
    runCommand(h, 'show');
    expect(shown).toEqual([
      { kind: 'tool', id: 'settings', section: 'keybindings' },
      { kind: 'tool', id: 'settings', section: 'appearance' },
      { kind: 'tool', id: 'settings', section: 'appearance' },
      { kind: 'tool', id: 'settings', section: 'vault' },
      { kind: 'help' },
      { kind: 'tool', id: 'ai' },
      { kind: 'apps' },
    ]);
  });

  test('pin, rescan, folders, launcher files and quit call the shell', async () => {
    const { host: h, backend, told } = host({ pinned: true });
    runCommand(h, 'pin');
    runCommand(h, 'rescan');
    runCommand(h, 'folder', { folder: 'documents' });
    runCommand(h, 'folder', { folder: 'C:\\Windows' });
    runCommand(h, 'config', { file: 'keybindings' });
    runCommand(h, 'config', { file: 'secrets' });
    runCommand(h, 'quit');
    await flush();
    expect(backend.setPinned).toHaveBeenCalledWith(false);
    expect(backend.rescan).toHaveBeenCalledTimes(1);
    expect(told).toEqual([['Apps rescanned', 'success']]);
    expect(backend.openFolder.mock.calls).toEqual([['documents']]);
    expect(backend.openConfigFile.mock.calls).toEqual([['keybindings']]);
    expect(backend.quit).toHaveBeenCalledTimes(1);
  });

  test('commands the launcher does not know go to the shell; failures reach the host', async () => {
    const { host: h, backend, told, failed } = host();
    runCommand(h, 'export', { to: 'zip' });
    await flush();
    expect(backend.runAction).toHaveBeenCalledWith('export', { to: 'zip' });
    expect(told).toEqual([['/export runs in the desktop app', 'success']]);

    const boom = new Error('disk gone');
    backend.setPinned.mockImplementation(() => Promise.reject(boom));
    runCommand(h, 'pin');
    await flush();
    expect(failed).toEqual([boom]);
  });
});

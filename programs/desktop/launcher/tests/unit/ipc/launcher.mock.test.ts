import { describe, expect, test } from 'bun:test';
import { createMockBackend, MOCK_VAULT_PASSWORD } from '../../../src/ipc/launcher.mock';
import { isVaultError } from '../../../src/ipc/launcher.parse';
import { VAULT_COMMANDS } from '../../../src/ipc/launcher.types';

describe('mock backend', () => {
  test('serves a plausible drive: three sources, favorites, recent and unavailable apps', async () => {
    const backend = createMockBackend();
    const list = await backend.listApps();
    expect(list.tabs.map((tab) => tab.source)).toEqual(['genslate', 'portapps', 'portableapps']);
    expect(list.apps.some((app) => app.favorite)).toBe(true);
    expect(list.recent.length).toBeGreaterThan(2);
    expect(
      list.apps.filter((app) => app.status !== 'ready' && app.status !== 'running').length,
    ).toBeGreaterThanOrEqual(2);
    const volume = await backend.volume();
    expect(volume?.name).toBe('SLATECORE');
    expect(volume?.availableBytes).toBeLessThan(volume?.totalBytes ?? 0);
  });

  test('the registry holds exactly the launcher slash commands', async () => {
    const context = await createMockBackend().context();
    expect(context.actions.map((action) => action.id)).toEqual([
      'open',
      'theme',
      'size',
      'pin',
      'settings',
      'vault',
      'rescan',
      'help',
      'ask',
    ]);
    expect(context.mode).toBe('web');
    expect(context.layout).toEqual({ inset: 16, normalWidth: 460, expandedWidth: 920 });
  });

  test('in-memory vault: locked, wrong password, unlock, list, lock', async () => {
    const backend = createMockBackend();
    expect((await backend.vaultStatus()).state).toBe('locked');
    const failure = await backend.vaultUnlock('nope').catch((error: unknown) => error);
    expect(isVaultError(failure) && failure.code).toBe('WRONG_PASSWORD');
    expect((await backend.vaultStatus()).failedAttempts).toBe(1);

    const status = await backend.vaultUnlock(MOCK_VAULT_PASSWORD);
    expect(status.state).toBe('unlocked');
    expect(status.failedAttempts).toBe(0);
    const root = await backend.vaultList('');
    expect(root.some((entry) => entry.kind === 'dir')).toBe(true);
    await backend.vaultLock();
    expect((await backend.vaultStatus()).state).toBe('locked');
  });

  test('repeated wrong passwords are throttled', async () => {
    const backend = createMockBackend();
    let last: unknown;
    for (let attempt = 0; attempt < 6; attempt += 1) {
      last = await backend.vaultUnlock('nope').catch((error: unknown) => error);
    }
    expect(isVaultError(last) && last.code).toBe('THROTTLED');
    expect((await backend.vaultStatus()).retryAfterMs).toBeGreaterThan(0);
  });

  test('a new vault rejects short passwords and starts unlocked', async () => {
    const backend = createMockBackend({ vault: 'uninitialized' });
    const short = await backend.vaultCreate('short').catch((error: unknown) => error);
    expect(isVaultError(short) && short.code).toBe('PASSWORD_REJECTED');
    expect((await backend.vaultCreate('aurora-borealis')).state).toBe('unlocked');
  });

  test('vault command names match research/vault.md §8.3', () => {
    expect(Object.values(VAULT_COMMANDS)).toEqual([
      'vault_status',
      'vault_create',
      'vault_unlock',
      'vault_lock',
      'vault_change_password',
      'vault_list',
      'vault_import',
      'vault_export',
      'vault_open',
      'vault_verify',
    ]);
  });
});

import { describe, expect, test } from 'bun:test';
import { createMockBackend, MOCK_VAULT_PASSWORD } from '../../../src/ipc/launcher.mock';
import { isVaultError } from '../../../src/ipc/launcher.parse';
import { VAULT_COMMANDS } from '../../../src/ipc/launcher.types';
import { BLOCKED_EXTENSIONS, isBlockedType } from '../../../src/ipc/open-policy.util';

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

  test('the registry holds every slash command of the shell, in its order', async () => {
    // tests/unit/app/rust-registry.test.ts compares it with actions.rs field by field.
    const context = await createMockBackend().context();
    expect(context.actions.map((action) => action.id)).toEqual([
      'open',
      'folder',
      'tab',
      'fav',
      'rescan',
      'theme',
      'size',
      'pin',
      'tools',
      'settings',
      'vault',
      'config',
      'keys',
      'logs',
      'ask',
      'help',
      'hide',
      'quit',
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

  test('vault passwords are 8 to 1024 bytes of UTF-8 after NFKC, like the crate', async () => {
    const rejected = async (password: string) => {
      const backend = createMockBackend({ vault: 'uninitialized' });
      return backend.vaultCreate(password).then(
        () => false,
        (error: unknown) => isVaultError(error) && error.code === 'PASSWORD_REJECTED',
      );
    };
    // Four accented letters are eight bytes; 512 of them are exactly 1024.
    expect(await rejected('é'.repeat(4))).toBe(false);
    expect(await rejected('é'.repeat(512))).toBe(false);
    expect(await rejected('é'.repeat(513))).toBe(true);
    // The ligature ﬀ is three bytes but normalises to "ff", two: nine bytes become six.
    expect(await rejected('ﬀﬀﬀ')).toBe(true);
    expect(await rejected('abcdefg')).toBe(true);
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

  test('vault_open refuses the code-bearing types of open_policy.rs, by name as Windows reads it', async () => {
    const policy = await Bun.file(
      new URL('../../../src-tauri/src/open_policy.rs', import.meta.url),
    ).text();
    const list = /const BLOCKED: &\[&str\] = &\[([\s\S]*?)\];/.exec(policy)?.[1] ?? '';
    const rust = [...list.matchAll(/"([^"]+)"/g)].map((match) => match[1] ?? '');
    expect(rust.length).toBeGreaterThan(40);
    expect([...BLOCKED_EXTENSIONS].sort()).toEqual(rust.sort());

    for (const name of [
      'setup.exe',
      'RUN.BAT',
      'backup.ps1',
      'notes.lnk',
      'report.pdf.exe',
      'evil.exe. ',
      '.exe',
      'a.txt:stream',
      'Documents\\tools\\setup.exe',
      'file.CHM',
      'disk.iso',
      'tool.vsix',
      // A non-ASCII character in the extension: not a type the list can vouch for.
      'report.\u0435xe',
      'note.t\uff58t',
      'a.\u202etxt',
    ]) {
      expect({ name, blocked: isBlockedType(name) }).toEqual({ name, blocked: true });
    }
    for (const name of [
      'ideas.md',
      'exe',
      'Photos/aurora.jpg',
      'exe.txt',
      'setup.exe/readme.txt',
      'folder.exe/',
      'budget.xlsm',
      'archive.tar.gz',
      '',
      'r\u00e9sum\u00e9.pdf',
      '\u65e5\u672c\u8a9e.txt',
    ]) {
      expect({ name, blocked: isBlockedType(name) }).toEqual({ name, blocked: false });
    }

    const backend = createMockBackend({ vault: 'unlocked' });
    const refused = await backend.vaultOpen('backup.ps1').catch((error: unknown) => error);
    expect(isVaultError(refused) && refused.code).toBe('BLOCKED_TYPE');
    expect((await backend.vaultStatus()).sessionFiles).toBe(0);
    await expect(backend.vaultOpen('ideas.md')).resolves.toBeDefined();
  });

  test('a blocked automatic lock: plain locks fail, a forced one drops the edits', async () => {
    const backend = createMockBackend({ vault: 'unlocked', lockBlocked: true });
    const notices: unknown[] = [];
    await backend.on('vaultLockBlocked', (notice) => notices.push(notice));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(notices).toEqual([
      {
        reason: 'hidden',
        error: {
          code: 'UNSYNCED_EDITS',
          message: '2 edited files could not be saved into the vault',
          paths: ['ideas.md', 'budget-2026.xlsx'],
        },
      },
    ]);
    const refused = await backend.vaultLock().catch((error: unknown) => error);
    expect(isVaultError(refused) && refused.code).toBe('UNSYNCED_EDITS');
    expect((await backend.vaultStatus()).state).toBe('unlocked');
    const report = await backend.vaultLock(true);
    expect(report.unsynced).toEqual(['ideas.md', 'budget-2026.xlsx']);
    expect((await backend.vaultStatus()).state).toBe('locked');
  });
});

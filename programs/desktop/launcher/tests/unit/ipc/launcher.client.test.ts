import { afterEach, describe, expect, test } from 'bun:test';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';

import { createBackend } from '../../../src/ipc/launcher.client';
import { PayloadError } from '../../../src/ipc/launcher.parse';

/** Pretends to run inside the desktop app, with `reply` answering every command. */
function inTauri(reply: (command: string) => unknown) {
  Reflect.set(globalThis, 'isTauri', true);
  mockIPC((command) => reply(command));
}

afterEach(() => {
  clearMocks();
  Reflect.deleteProperty(globalThis, 'isTauri');
});

describe('Tauri backend', () => {
  test('malformed command results reject with a PayloadError, not a broken object', async () => {
    inTauri(() => ({ unexpected: true }));
    const backend = await createBackend();
    await expect(backend.vaultLock()).rejects.toBeInstanceOf(PayloadError);
    await expect(backend.vaultVerify()).rejects.toBeInstanceOf(PayloadError);
    await expect(backend.vaultOpen('ideas.md')).rejects.toBeInstanceOf(PayloadError);
    await expect(backend.runAction('rescan', {})).rejects.toBeInstanceOf(PayloadError);
  });

  test('well-formed results come back typed', async () => {
    const replies: Readonly<Record<string, unknown>> = {
      vault_lock: { synced: [], unsynced: ['ideas.md'], wipedFiles: 1, undeletable: [] },
      vault_verify: { filesChecked: 3, orphanBlobs: 0, problems: [] },
      vault_open: { sessionPath: 'other/launcher/cache/vault-session/ideas.md' },
      run_action: { kind: 'done', message: null },
    };
    inTauri((command) => replies[command]);
    const backend = await createBackend();
    expect((await backend.vaultLock()).unsynced).toEqual(['ideas.md']);
    expect((await backend.vaultVerify()).filesChecked).toBe(3);
    expect((await backend.vaultOpen('ideas.md')).sessionPath).toContain('vault-session');
    expect(await backend.runAction('rescan', {})).toEqual({ kind: 'done', message: null });
  });
});

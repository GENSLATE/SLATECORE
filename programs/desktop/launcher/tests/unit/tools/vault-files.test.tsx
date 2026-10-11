import { describe, expect, test } from 'bun:test';
import { screen, waitFor, within } from '@testing-library/react';

import { renderLauncher } from '../launcher.harness';

const search = () => screen.getByRole('combobox', { name: /Search apps/ });
const settings = () => within(screen.getByRole('region', { name: 'Settings' }));
const file = async (name: string) =>
  settings().findByRole('button', { name: new RegExp(`^${name.replace('.', '\\.')}`) });

const blockedLock = () => settings().getByRole('alert', { name: "Couldn't lock automatically" });
const BLOCKED_COPY =
  "That file type can run code, so it can't be opened from the vault. Export it to a folder instead.";

describe('vault files', () => {
  test('the first open in a session explains the traces it leaves, once', async () => {
    const { backend, user } = await renderLauncher({ vault: 'unlocked' });
    await user.type(search(), '/vault{Enter}');
    await user.click(await file('ideas.md'));
    // Nothing is decrypted until the note has been read.
    expect(backend.vaultOpen).not.toHaveBeenCalled();
    const note = settings().getByRole('status', { name: 'Opening leaves traces on this PC' });
    expect(note).toHaveTextContent(/Windows Explorer/);
    expect(note).toHaveTextContent(/Recent Items/);
    expect(note).toHaveTextContent(/Export leaves nothing behind/);

    await user.click(within(note).getByRole('button', { name: 'Open ideas.md' }));
    expect(backend.vaultOpen).toHaveBeenCalledWith('ideas.md');
    expect(
      settings().queryByRole('status', { name: 'Opening leaves traces on this PC' }),
    ).toBeNull();

    // Later opens in the same session go straight through.
    await user.click(await file('budget-2026.xlsx'));
    expect(backend.vaultOpen).toHaveBeenLastCalledWith('budget-2026.xlsx');
    expect(
      settings().queryByRole('status', { name: 'Opening leaves traces on this PC' }),
    ).toBeNull();
  });

  test('dismissing the note counts as read; Not now opens nothing', async () => {
    const { backend, user } = await renderLauncher({ vault: 'unlocked' });
    await user.type(search(), '/vault{Enter}');
    await user.click(await file('ideas.md'));
    const note = () => settings().getByRole('status', { name: 'Opening leaves traces on this PC' });
    await user.click(within(note()).getByRole('button', { name: 'Not now' }));
    expect(backend.vaultOpen).not.toHaveBeenCalled();

    await user.click(await file('ideas.md'));
    await user.click(within(note()).getByRole('button', { name: 'Dismiss' }));
    expect(backend.vaultOpen).not.toHaveBeenCalled();
    await user.click(await file('ideas.md'));
    expect(backend.vaultOpen).toHaveBeenCalledWith('ideas.md');
  });

  test('a file type that can run code is refused with a clear message', async () => {
    const { backend, user } = await renderLauncher({ vault: 'unlocked' });
    await user.type(search(), '/vault{Enter}');
    await user.click(await file('backup.ps1'));
    // The shell decides; the code-bearing type never needs the traces note first.
    expect(backend.vaultOpen).toHaveBeenCalledWith('backup.ps1');
    const notice = await settings().findByRole('alert', { name: "Can't open backup.ps1" });
    expect(notice).toHaveTextContent(BLOCKED_COPY);
    expect(screen.queryByText(/Opened backup\.ps1/)).toBeNull();
    await user.click(within(notice).getByRole('button', { name: 'Dismiss' }));
    expect(settings().queryByRole('alert', { name: "Can't open backup.ps1" })).toBeNull();
  });

  test('a blocked automatic lock shows a lasting notice and keeps the vault unlocked', async () => {
    const { backend, emit, user } = await renderLauncher({ vault: 'unlocked', lockBlocked: true });
    await user.type(search(), '/vault{Enter}');
    emit('vaultLockBlocked', {
      reason: 'idle',
      error: {
        code: 'UNSYNCED_EDITS',
        message: 'edits could not be saved',
        paths: ['ideas.md', 'budget-2026.xlsx'],
      },
    });
    const notice = await settings().findByRole('alert', { name: "Couldn't lock automatically" });
    expect(notice).toHaveTextContent(
      "Couldn't lock automatically: 2 edited files couldn't be saved. Close the program that has them open, then lock again.",
    );
    expect(settings().getByText('Unlocked')).toBeInTheDocument();

    // Lock again: the edits still cannot be saved, so the vault stays unlocked.
    await user.click(within(notice).getByRole('button', { name: 'Lock again' }));
    expect(backend.vaultLock).toHaveBeenLastCalledWith();
    await waitFor(() => expect(backend.vaultLock).toHaveBeenCalledTimes(1));
    expect(blockedLock()).toBeInTheDocument();
    expect(settings().getByText('Unlocked')).toBeInTheDocument();

    // Discarding needs a second, explicit step.
    await user.click(
      within(blockedLock()).getByRole('button', { name: 'Lock and discard unsaved edits' }),
    );
    expect(backend.vaultLock).toHaveBeenCalledTimes(1);
    const confirm = blockedLock();
    expect(confirm).toHaveTextContent(/edits to ideas\.md and budget-2026\.xlsx are lost/i);
    await user.click(within(confirm).getByRole('button', { name: 'Discard edits and lock' }));
    expect(backend.vaultLock).toHaveBeenLastCalledWith(true);
    await waitFor(() => expect(settings().getByText('Locked')).toBeInTheDocument());
    expect(settings().queryByRole('alert', { name: "Couldn't lock automatically" })).toBeNull();
  });

  test('one edited file reads in the singular; Cancel keeps the edits', async () => {
    const { backend, emit, user } = await renderLauncher({ vault: 'unlocked', lockBlocked: true });
    await user.type(search(), '/vault{Enter}');
    emit('vaultLockBlocked', {
      reason: 'hidden',
      error: { code: 'UNSYNCED_EDITS', message: 'edits could not be saved', paths: ['ideas.md'] },
    });
    const notice = await settings().findByRole('alert', { name: "Couldn't lock automatically" });
    expect(notice).toHaveTextContent("1 edited file couldn't be saved.");
    await user.click(
      within(notice).getByRole('button', { name: 'Lock and discard unsaved edits' }),
    );
    await user.click(within(blockedLock()).getByRole('button', { name: 'Cancel' }));
    expect(backend.vaultLock).not.toHaveBeenCalled();
    expect(within(blockedLock()).getByRole('button', { name: 'Lock again' })).toBeInTheDocument();
  });
});

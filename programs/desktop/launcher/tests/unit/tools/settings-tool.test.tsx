// cspell:ignore borealiz
import { describe, expect, test } from 'bun:test';
import { readdir } from 'node:fs/promises';
import { screen, waitFor, within } from '@testing-library/react';

import { MOCK_VAULT_PASSWORD } from '../../../src/ipc/launcher.mock';
import { renderLauncher, renderTrayMenu, settle } from '../launcher.harness';
import { reactStateContains } from '../react-state.util';

const search = () => screen.getByRole('combobox', { name: /Search apps/ });
const settings = () => screen.getByRole('region', { name: 'Settings' });
const sectionTab = (name: string) =>
  within(screen.getByRole('tablist', { name: 'Settings sections' })).getByRole('tab', { name });

/** Startup-with-the-OS controls: none may exist anywhere (leave no trace). */
const STARTUP =
  /autostart|auto-start|start (with|on) (system|windows|login|startup)|launch (at|on) (login|startup)|run at startup/i;

function startupControls(): HTMLElement[] {
  const roles = ['switch', 'checkbox', 'menuitemcheckbox', 'menuitem', 'button', 'option', 'tab'];
  return roles
    .flatMap((role) => screen.queryAllByRole(role))
    .filter(
      (element) =>
        STARTUP.test(element.textContent ?? '') ||
        STARTUP.test(element.getAttribute('aria-label') ?? ''),
    );
}

async function sourceFiles(dir: URL): Promise<URL[]> {
  const entries = await readdir(dir, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map((entry) =>
      entry.isDirectory()
        ? sourceFiles(new URL(`${entry.name}/`, dir))
        : Promise.resolve([new URL(entry.name, dir)]),
    ),
  );
  return nested.flat();
}

describe('Settings tool', () => {
  test('slash_settings_and_vault_open_the_settings_tool', async () => {
    const { user } = await renderLauncher();
    await user.type(search(), '/settings{Enter}');
    expect(settings()).toBeInTheDocument();
    expect(sectionTab('Appearance')).toHaveAttribute('aria-selected', 'true');

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('region', { name: 'Settings' })).toBeNull();

    await user.type(search(), '/vault{Enter}');
    expect(settings()).toBeInTheDocument();
    expect(sectionTab('Vault')).toHaveAttribute('aria-selected', 'true');
  });

  test('ai_tool_and_slash_ask_show_feature_teaser', async () => {
    const { user } = await renderLauncher();
    await user.click(screen.getByRole('button', { name: 'AI tools' }));
    const teaser = screen.getByRole('heading', { name: 'AI tools', level: 2 });
    expect(teaser.closest('[data-slot="feature-teaser"]')).toHaveTextContent('Coming soon');

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('heading', { name: 'AI tools', level: 2 })).toBeNull();

    await user.type(search(), '/ask{Enter}');
    expect(screen.getByRole('heading', { name: 'AI tools', level: 2 })).toBeInTheDocument();
  });

  test('other tools render feature teasers', async () => {
    const { user } = await renderLauncher();
    for (const name of ['App manager', 'Storage & backup', 'Diagnostics']) {
      await user.click(screen.getByRole('button', { name }));
      const heading = screen.getByRole('heading', { name, level: 2 });
      expect(heading.closest('[data-slot="feature-teaser"]')).toHaveTextContent('Coming soon');
    }
  });

  test('settings_has_appearance_behavior_keybindings_vault_about_sections', async () => {
    const { user } = await renderLauncher();
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    const tabs = within(screen.getByRole('tablist', { name: 'Settings sections' })).getAllByRole(
      'tab',
    );
    expect(tabs.map((tab) => tab.textContent)).toEqual([
      'Appearance',
      'Behavior',
      'Keybindings',
      'Vault',
      'About this drive',
    ]);
  });

  test('settings_theme_change_calls_backend_and_updates_theme', async () => {
    const { backend, user } = await renderLauncher({ theme: 'polar-night' });
    expect(document.documentElement.dataset['theme']).toBe('polar-night');
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    await user.click(within(settings()).getByRole('radio', { name: 'Snow Storm' }));
    expect(backend.setSetting).toHaveBeenCalledWith('theme', 'snow-storm');
    await waitFor(() => expect(document.documentElement.dataset['theme']).toBe('snow-storm'));
  });

  test('theme_picker_previews_each_theme_and_marks_the_current_one', async () => {
    const { backend, user } = await renderLauncher({ theme: 'snow-storm' });
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    const picker = within(settings()).getByRole('radiogroup', { name: 'Color theme' });
    const options = within(picker).getAllByRole('radio');
    expect(options.map((option) => option.closest('label')?.textContent)).toEqual([
      'Polar Night',
      'Snow Storm',
      'System',
    ]);
    expect(options.map((option) => (option as HTMLInputElement).checked)).toEqual([
      false,
      true,
      false,
    ]);
    // Each card previews its theme with the theme's own tokens.
    const previews = [...picker.querySelectorAll('[data-slot="theme-preview"]')];
    expect(previews.map((preview) => preview.getAttribute('data-theme'))).toEqual([
      'polar-night',
      'snow-storm',
      null,
    ]);
    // One tab stop; the arrow keys move the choice, as in any radio group.
    options[1]?.focus();
    await user.keyboard('{ArrowRight}');
    expect(backend.setSetting).toHaveBeenLastCalledWith('theme', 'system');
  });

  test('size presets write the size setting', async () => {
    const { backend, user } = await renderLauncher({ size: 'm' });
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    await user.click(within(settings()).getByRole('button', { name: /^Large/ }));
    expect(backend.setSetting).toHaveBeenCalledWith('size', 'l');
    await waitFor(() =>
      expect(document.querySelector('[data-slot="launcher-window"]')).toHaveAttribute(
        'data-size',
        'l',
      ),
    );
  });

  test('vault_section_locked_by_default_and_unlock_flow_with_wrong_password_error', async () => {
    const { backend, user } = await renderLauncher();
    await user.type(search(), '/vault{Enter}');
    const panel = within(settings());
    expect(panel.getByText('Locked')).toBeInTheDocument();

    const field = panel.getByLabelText('Vault password');
    await user.type(field, 'not-the-password');
    await user.click(panel.getByRole('button', { name: 'Unlock' }));
    expect(await panel.findByText(/wrong password/i)).toBeInTheDocument();
    expect(field).toHaveValue('');
    expect(backend.vaultUnlock).toHaveBeenCalledTimes(1);

    await user.type(field, MOCK_VAULT_PASSWORD);
    await user.click(panel.getByRole('button', { name: 'Unlock' }));
    expect(await panel.findByText('Unlocked')).toBeInTheDocument();
    expect(panel.getByRole('button', { name: 'Lock now' })).toBeInTheDocument();
    expect(await panel.findByText('ideas.md')).toBeInTheDocument();

    await user.click(panel.getByRole('button', { name: 'Lock now' }));
    expect(await panel.findByText('Locked')).toBeInTheDocument();
  });

  test('vault_section_warns_no_recovery', async () => {
    const { user } = await renderLauncher({ vault: 'uninitialized' });
    await user.type(search(), '/vault{Enter}');
    const panel = within(settings());
    expect(panel.getByText(/no password recovery/i)).toBeInTheDocument();
    expect(panel.getByLabelText('New vault password')).toBeInTheDocument();
    expect(panel.getByLabelText('Repeat the password')).toBeInTheDocument();
  });

  test('creating a vault checks the password twice', async () => {
    const { backend, user } = await renderLauncher({ vault: 'uninitialized' });
    await user.type(search(), '/vault{Enter}');
    const panel = within(settings());
    await user.type(panel.getByLabelText('New vault password'), 'aurora-borealis');
    await user.type(panel.getByLabelText('Repeat the password'), 'aurora-borealiz');
    await user.click(panel.getByRole('button', { name: 'Create vault' }));
    expect(panel.getByText(/passwords do not match/i)).toBeInTheDocument();
    expect(backend.vaultCreate).not.toHaveBeenCalled();

    await user.type(panel.getByLabelText('Repeat the password'), 'aurora-borealis');
    await user.click(panel.getByRole('button', { name: 'Create vault' }));
    expect(backend.vaultCreate).toHaveBeenCalledWith('aurora-borealis');
    expect(await panel.findByText('Unlocked')).toBeInTheDocument();
  });

  test('no_password_is_retained_after_submit', async () => {
    const created = await renderLauncher({ vault: 'uninitialized' });
    await created.user.type(search(), '/vault{Enter}');
    let panel = within(settings());
    await created.user.type(panel.getByLabelText('New vault password'), 'aurora-borealis');
    await created.user.type(panel.getByLabelText('Repeat the password'), 'aurora-borealis');
    // While typed, the secret is in the inputs only: no component state holds a copy.
    expect(reactStateContains(created.container, 'aurora-borealis')).toBe(false);
    await created.user.keyboard('{Enter}');
    expect(created.backend.vaultCreate).toHaveBeenCalledWith('aurora-borealis');
    expect(await panel.findByText('Unlocked')).toBeInTheDocument();
    expect(reactStateContains(created.container, 'aurora-borealis')).toBe(false);
    created.unmount();

    const { backend, container, user } = await renderLauncher();
    await user.type(search(), '/vault{Enter}');
    panel = within(settings());
    const field = panel.getByLabelText('Vault password');
    await user.type(field, 'not-the-password');
    expect(reactStateContains(container, 'not-the-password')).toBe(false);
    await user.keyboard('{Enter}');
    expect(await panel.findByText(/wrong password/i)).toBeInTheDocument();
    expect(field).toHaveValue('');
    expect(reactStateContains(container, 'not-the-password')).toBe(false);

    await user.type(field, `${MOCK_VAULT_PASSWORD}{Enter}`);
    expect(await panel.findByText('Unlocked')).toBeInTheDocument();
    await user.type(panel.getByLabelText('Current password'), MOCK_VAULT_PASSWORD);
    await user.type(panel.getByLabelText('New password'), 'polar-night-77');
    await user.type(panel.getByLabelText('Repeat new password'), 'polar-night-77');
    expect(reactStateContains(container, 'polar-night-77')).toBe(false);
    await user.click(panel.getByRole('button', { name: 'Change password' }));
    expect(backend.vaultChangePassword).toHaveBeenCalledWith(MOCK_VAULT_PASSWORD, 'polar-night-77');
    expect(await screen.findByText('Vault password changed')).toBeInTheDocument();
    for (const label of ['Current password', 'New password', 'Repeat new password']) {
      expect(panel.getByLabelText(label)).toHaveValue('');
    }
    expect(reactStateContains(container, MOCK_VAULT_PASSWORD)).toBe(false);
    expect(reactStateContains(container, 'polar-night-77')).toBe(false);
  });

  test('a wrong current password is reported under Current and keeps the new ones', async () => {
    const { backend, user } = await renderLauncher({ vault: 'unlocked' });
    await user.type(search(), '/vault{Enter}');
    const panel = within(settings());
    const current = panel.getByLabelText('Current password');
    const next = panel.getByLabelText('New password');
    const repeat = panel.getByLabelText('Repeat new password');
    await user.type(current, 'not-it');
    await user.type(next, 'polar-night-77');
    await user.type(repeat, 'polar-night-77');
    await user.click(panel.getByRole('button', { name: 'Change password' }));
    expect(backend.vaultChangePassword).toHaveBeenCalledTimes(1);

    expect(await panel.findByText(/wrong password/i)).toBeInTheDocument();
    expect(current).toHaveAttribute('aria-invalid', 'true');
    expect(current).toHaveAccessibleDescription(/wrong password/i);
    expect(current).toHaveValue('');
    expect(next).toHaveValue('polar-night-77');
    expect(repeat).toHaveValue('polar-night-77');
    expect(next).not.toHaveAttribute('aria-invalid');

    await user.type(current, 'a');
    expect(panel.queryByText(/wrong password/i)).toBeNull();
    expect(current).not.toHaveAttribute('aria-invalid');
  });

  test('vault password length counts UTF-8 bytes after NFKC', async () => {
    const { backend, user } = await renderLauncher({ vault: 'uninitialized' });
    await user.type(search(), '/vault{Enter}');
    const panel = within(settings());
    const password = panel.getByLabelText('New vault password');
    const repeat = panel.getByLabelText('Repeat the password');
    // Seven ASCII bytes: too short.
    await user.type(password, 'abcdefg');
    await user.type(repeat, 'abcdefg{Enter}');
    expect(await panel.findByText(/at least 8/i)).toBeInTheDocument();
    expect(password).toHaveAccessibleDescription(/at least 8/i);
    expect(backend.vaultCreate).not.toHaveBeenCalled();
    // Four accented letters are eight bytes: long enough, though only four characters.
    await user.clear(password);
    await user.clear(repeat);
    const accented = 'é'.repeat(4);
    await user.type(password, accented);
    await user.type(repeat, `${accented}{Enter}`);
    expect(backend.vaultCreate).toHaveBeenCalledWith(accented);
    expect(await panel.findByText('Unlocked')).toBeInTheDocument();
  });

  test('the vault file list shows an error state when it cannot be read', async () => {
    const { backend, user } = await renderLauncher({ vault: 'unlocked' });
    const list = backend.vaultList.getMockImplementation();
    backend.vaultList.mockImplementation(() =>
      Promise.reject({ code: 'IO', message: 'The drive stopped answering.' }),
    );
    await user.type(search(), '/vault{Enter}');
    const panel = within(settings());
    expect(await panel.findByText('Could not read the vault')).toBeInTheDocument();
    expect(panel.getByText('The drive stopped answering.')).toBeInTheDocument();
    expect(panel.queryByText('Nothing here yet')).toBeNull();

    if (list !== undefined) backend.vaultList.mockImplementation(list);
    await user.click(panel.getByRole('button', { name: 'Try again' }));
    expect(await panel.findByText('ideas.md')).toBeInTheDocument();
  });

  test('locked_vault_rail_entry_opens_settings_vault_section', async () => {
    const { user } = await renderLauncher();
    await user.click(screen.getByRole('button', { name: 'Vault (locked)' }));
    expect(settings()).toBeInTheDocument();
    expect(sectionTab('Vault')).toHaveAttribute('aria-selected', 'true');
  });

  test('about_section_shows_slatecore_launcher_by_genslate_and_version', async () => {
    const { user } = await renderLauncher();
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    await user.click(sectionTab('About this drive'));
    const panel = within(settings());
    expect(panel.getByRole('heading', { name: 'SLATECORE LAUNCHER' })).toBeInTheDocument();
    expect(panel.getByText('by GENSLATE')).toBeInTheDocument();
    expect(panel.getByText('Version 0.1.0')).toBeInTheDocument();
  });

  test('about_this_drive_states_what_windows_and_other_launchers_may_still_record', async () => {
    const { user } = await renderLauncher();
    await user.type(search(), '/settings about{Enter}');
    const limits = within(settings()).getByRole('region', {
      name: 'What this PC may still record',
    });
    expect(limits).toHaveTextContent(/Windows/);
    expect(limits).toHaveTextContent(/Recent files/);
    expect(limits).toHaveTextContent(/Prefetch/);
    expect(limits).toHaveTextContent(/notification area/);
    expect(limits).toHaveTextContent(/PortableApps\.com and portapps\.io launchers/);
  });

  test('no_autostart_control_anywhere', async () => {
    const { user, unmount } = await renderLauncher();
    expect(startupControls()).toEqual([]);
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    for (const name of ['Appearance', 'Behavior', 'Keybindings', 'Vault', 'About this drive']) {
      await user.click(sectionTab(name));
      expect(startupControls()).toEqual([]);
    }
    unmount();

    await renderTrayMenu();
    await user.click(await screen.findByRole('menuitem', { name: 'Settings' }));
    await screen.findByRole('menu', { name: 'Settings' });
    await settle();
    expect(startupControls()).toEqual([]);

    const files = await sourceFiles(new URL('../../../src/', import.meta.url));
    const offenders: string[] = [];
    for (const file of files) {
      if (/autostart/i.test(await Bun.file(file).text())) offenders.push(file.pathname);
    }
    expect(offenders).toEqual([]);
  });
});

import { describe, expect, test } from 'bun:test';
import { screen, within } from '@testing-library/react';

import { renderLauncher, settle } from '../launcher.harness';

const search = () => screen.getByRole('combobox', { name: /Search apps/ });

/** The ids the combobox says it controls (none when the attribute is absent). */
function controlled(): string[] {
  return (search().getAttribute('aria-controls') ?? '').split(' ').filter(Boolean);
}

describe('app list semantics', () => {
  test('listboxes hold only options; group headers are buttons outside them', async () => {
    const { user } = await renderLauncher();
    for (const header of document.querySelectorAll('[data-slot="app-group-header"]')) {
      expect(header.closest('[role="listbox"]')).toBeNull();
    }
    for (const option of screen.getAllByRole('option')) {
      expect(within(option).queryAllByRole('button')).toEqual([]);
    }

    const header = screen.getByRole('button', { name: /^Favorites/ });
    const list = document.getElementById(header.getAttribute('aria-controls') ?? '');
    expect(list).toHaveAttribute('role', 'listbox');
    expect(list).toHaveAccessibleName('Favorites');
    expect(
      within(screen.getByRole('listbox', { name: 'Favorites' })).getAllByRole('option').length,
    ).toBeGreaterThan(0);

    // A collapsed group shows no listbox, so its header controls nothing.
    const unavailable = screen.getByRole('button', { name: /^Unavailable/ });
    expect(unavailable).not.toHaveAttribute('aria-controls');
    await user.click(unavailable);
    expect(screen.getByRole('listbox', { name: 'Unavailable' })).toBeInTheDocument();
  });

  test('the search box controls only listboxes that are on screen', async () => {
    const { user } = await renderLauncher();
    const ids = controlled();
    expect(ids.length).toBeGreaterThan(1);
    for (const id of ids) expect(document.getElementById(id)).toHaveAttribute('role', 'listbox');
    expect(search()).toHaveAttribute('aria-expanded', 'true');

    await user.type(search(), '/');
    expect(controlled()).toEqual([screen.getByRole('listbox', { name: 'Commands' }).id]);
    await user.type(search(), 'zzzz');
    expect(search()).not.toHaveAttribute('aria-controls');
    expect(search()).toHaveAttribute('aria-expanded', 'false');

    await user.clear(search());
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    expect(search()).not.toHaveAttribute('aria-controls');
    expect(search()).toHaveAttribute('aria-expanded', 'false');
    expect(search()).not.toHaveAttribute('aria-activedescendant');
  });

  test('an empty drive leaves the search box controlling nothing', async () => {
    await renderLauncher({ apps: [] });
    expect(search()).not.toHaveAttribute('aria-controls');
    expect(search()).toHaveAttribute('aria-expanded', 'false');
  });

  test('the favorite star stays a pointer shortcut, out of the option semantics', async () => {
    const { backend, user } = await renderLauncher();
    const option = screen.getAllByRole('option', { name: /Terminal/ })[0];
    if (option === undefined) throw new Error('no Terminal option');
    expect(within(option).queryAllByRole('button')).toEqual([]);
    const star = option.querySelector('[data-slot="icon-button"]');
    if (!(star instanceof HTMLElement)) throw new Error('no star');
    await user.click(star);
    expect(backend.setOverride).toHaveBeenCalledWith('genslate/terminal', { favorite: false });
    await settle();
  });
});

describe('command bar', () => {
  test('no empty keycap when the focus shortcut is unbound', async () => {
    const { backend, emit } = await renderLauncher();
    const bar = () => document.querySelector('[data-slot="command-bar"]');
    expect(bar()?.querySelector('[data-slot="kbd"]')).not.toBeNull();
    const { settings } = await backend.context();
    emit('settings', {
      ...settings,
      keybindings: {
        ...settings.keybindings,
        launcher: { ...settings.keybindings.launcher, focusSearch: '' },
      },
    });
    expect(bar()?.querySelector('[data-slot="kbd"]')).toBeNull();
  });

  test('settings, vault and help each have their own icon in the command list', async () => {
    const { user } = await renderLauncher();
    await user.type(search(), '/');
    const list = screen.getByRole('listbox', { name: 'Commands' });
    const glyph = (id: string) => {
      const option = within(list).getByRole('option', { name: new RegExp(`^/${id}\\b`) });
      return /codicon-([\w-]+)/.exec(
        option.querySelector('[data-slot="icon"]')?.className ?? '',
      )?.[1];
    };
    const icons = ['settings', 'vault', 'help'].map(glyph);
    expect(icons.every((icon) => icon !== undefined)).toBe(true);
    expect(new Set(icons).size).toBe(3);
  });
});

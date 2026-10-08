/**
 * `covers` is hand-written, so these tests check that each claim is real: the component is on
 * its page (a `data-slot` marker in the rendered page, or JSX in the page's source when the
 * component only exists while an overlay is open), and the Menu and ContextMenu pages really
 * render every row type they claim once opened.
 */
import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { DesignSystemProvider, ToastProvider, TooltipProvider } from '@genslate/design-system';
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { SHOWCASE_SECTIONS } from '../../src/features/showcase/showcase.registry';

const kebab = (name: string) => name.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();

/** Components whose `data-slot` is not their kebab-cased name. */
const SLOT: Readonly<Record<string, string>> = {
  Tab: 'tabs-tab',
  TitleBar: 'titlebar',
  TitleBarCommandCenter: 'titlebar-command-center',
  StatusBar: 'statusbar',
  StatusBarItem: 'statusbar-item',
  StatusBarSection: 'statusbar-section',
  TextField: 'text-field-control',
  SearchField: 'search-field-hint',
  PasswordField: 'password-field-toggle',
  ToolbarButton: 'icon-button',
  ToolbarTextButton: 'button',
  // The ContextMenu rows are the Menu rows under another name.
  ContextMenuCheckboxItem: 'menu-checkbox-item',
  ContextMenuGroup: 'menu-group',
  ContextMenuGroupLabel: 'menu-group-label',
  ContextMenuItem: 'menu-item',
  ContextMenuRadioGroup: 'menu-radio-group',
  ContextMenuRadioItem: 'menu-radio-item',
  ContextMenuSeparator: 'menu-separator',
  ContextMenuSubmenuTrigger: 'menu-submenu-trigger',
};

/** Providers: no DOM of their own. They are checked where the Kit mounts them instead. */
const PROVIDERS = new Set([
  'DesignSystemProvider',
  'ThemeProvider',
  'PlatformProvider',
  'WindowStateProvider',
  'CursorProvider',
  'TooltipProvider',
  'ToastProvider',
  'ToastViewport',
]);

/**
 * Components with no marker in a closed page: roots without DOM, and parts of portalled overlays
 * that exist only while open. They must still appear as JSX in the page's source files, and the
 * Menu and ContextMenu ones are rendered open in the second block below.
 */
const NO_CLOSED_MARKER = new Set([
  'WindowContextMenu',
  'Select',
  'SelectPopup',
  'SelectItem',
  'SelectGroup',
  'SelectGroupLabel',
  'SelectSeparator',
  'Tooltip',
  'Popover',
  'PopoverPopup',
  'PopoverTitle',
  'PopoverDescription',
  'PopoverClose',
  'Menu',
  'MenuPopup',
  'MenuItem',
  'MenuCheckboxItem',
  'MenuGroup',
  'MenuGroupLabel',
  'MenuHeader',
  'MenuRadioGroup',
  'MenuRadioItem',
  'MenuSeparator',
  'MenuShortcut',
  'MenuSubmenuRoot',
  'MenuSubmenuTrigger',
  'ContextMenu',
  'ContextMenuPopup',
  'ContextMenuCheckboxItem',
  'ContextMenuGroup',
  'ContextMenuGroupLabel',
  'ContextMenuItem',
  'ContextMenuRadioGroup',
  'ContextMenuRadioItem',
  'ContextMenuSeparator',
  'ContextMenuSubmenuRoot',
  'ContextMenuSubmenuTrigger',
  'Dialog',
  'DialogPopup',
  'DialogTitle',
  'DialogDescription',
  'DialogBody',
  'DialogFooter',
  'DialogClose',
  'AlertDialog',
  'AlertDialogPopup',
  'AlertDialogTitle',
  'AlertDialogDescription',
  'AlertDialogFooter',
  'AlertDialogClose',
  'CommandPalette',
]);

/** Source of the category's page files; code samples in template strings are not usage. */
function pageSource(category: string): string {
  const dir = new URL(`../../src/features/showcase/sections/${category}/`, import.meta.url);
  return readdirSync(dir)
    .filter((file) => file.endsWith('.tsx'))
    .map((file) => readFileSync(new URL(file, dir), 'utf8'))
    .join('\n')
    .replace(/`(?:[^`\\]|\\.)*`/gs, '``');
}

function Providers({ children }: { children: ReactNode }) {
  return (
    <DesignSystemProvider platform="linux" theme="polar-night">
      <TooltipProvider>
        <ToastProvider>{children}</ToastProvider>
      </TooltipProvider>
    </DesignSystemProvider>
  );
}

const slotsIn = (root: ParentNode) =>
  new Set([...root.querySelectorAll('[data-slot]')].map((el) => el.getAttribute('data-slot')));

const sectionOf = (id: string) => {
  const section = SHOWCASE_SECTIONS.find((entry) => entry.id === id);
  if (!section) throw new Error(`no showcase page "${id}"`);
  return section;
};

describe('showcase covers are rendered, not just listed', () => {
  test('every exemption names a component some page covers', () => {
    const covered = new Set(SHOWCASE_SECTIONS.flatMap((section) => section.covers));
    const exempt = [...PROVIDERS, ...NO_CLOSED_MARKER, ...Object.keys(SLOT)];
    expect(exempt.filter((name) => !covered.has(name))).toEqual([]);
  });

  test('the Kit mounts the providers once, in AppProviders', () => {
    const source = readFileSync(
      new URL('../../src/app/app.providers.tsx', import.meta.url),
      'utf8',
    );
    for (const name of [
      'DesignSystemProvider',
      'TooltipProvider',
      'ToastProvider',
      'ToastViewport',
    ]) {
      expect(source).toContain(`<${name}`);
    }
  });

  for (const section of SHOWCASE_SECTIONS) {
    test(`${section.id}: each covered component is on the page`, async () => {
      const Page = section.component;
      render(
        <Providers>
          <Page />
        </Providers>,
      );
      await act(async () => {});
      const present = slotsIn(document.body);
      const source = pageSource(section.category);

      const noMarker: string[] = [];
      const notInSource: string[] = [];
      for (const name of section.covers) {
        if (PROVIDERS.has(name)) continue;
        if (!NO_CLOSED_MARKER.has(name) && !present.has(SLOT[name] ?? kebab(name))) {
          noMarker.push(name);
        }
        if (!new RegExp(`<${name}[\\s/>]`).test(source)) notInSource.push(name);
      }
      expect({ noMarker, notInSource }).toEqual({ noMarker: [], notInSource: [] });
    });
  }
});

describe('open overlays render every row type their page claims', () => {
  test('Menu page: the File, Edit and Account menus render each Menu part', async () => {
    const user = userEvent.setup();
    const Page = sectionOf('menu').component;
    render(
      <Providers>
        <Page />
      </Providers>,
    );
    await act(async () => {});
    const seen = new Set<string | null>();
    for (const name of ['File', 'Edit', 'Account']) {
      await user.click(screen.getByRole('button', { name: new RegExp(`^${name}`) }));
      await screen.findByRole('menu');
      for (const slot of slotsIn(document.body)) seen.add(slot);
      await user.keyboard('{Escape}');
      await act(async () => {});
    }
    const expected = sectionOf('menu')
      .covers.filter((name) => name !== 'Menu' && name !== 'MenuSubmenuRoot')
      .map((name) => SLOT[name] ?? kebab(name));
    expect(expected.filter((slot) => !seen.has(slot))).toEqual([]);
  });

  test('Context Menu page: right-clicking each area renders every ContextMenu part', async () => {
    const user = userEvent.setup();
    const Page = sectionOf('context-menu').component;
    render(
      <Providers>
        <Page />
      </Providers>,
    );
    await act(async () => {});
    const seen = new Set<string | null>();
    const areas = [...document.querySelectorAll('[data-slot="context-menu-trigger"]')];
    expect(areas.length).toBeGreaterThanOrEqual(2);
    for (const area of areas) {
      await user.pointer({ keys: '[MouseRight]', target: area });
      await screen.findByRole('menu');
      for (const slot of slotsIn(document.body)) seen.add(slot);
      await user.keyboard('{Escape}');
      await act(async () => {});
    }
    const expected = sectionOf('context-menu')
      .covers.filter((name) => name !== 'ContextMenu' && name !== 'ContextMenuSubmenuRoot')
      .map((name) => SLOT[name] ?? kebab(name));
    expect(expected.filter((slot) => !seen.has(slot))).toEqual([]);
  });
});

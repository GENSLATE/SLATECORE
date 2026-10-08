import type { ShowcaseSection } from '../../showcase.types';
import { AlertDialogSection } from './alert-dialog.section';
import { CommandPaletteSection } from './command-palette.section';
import { ContextMenuSection } from './context-menu.section';
import { DialogSection } from './dialog.section';
import { MenuSection } from './menu.section';
import { PopoverSection } from './popover.section';
import { ToastSection } from './toast.section';
import { TooltipSection } from './tooltip.section';

/** Showcase pages for the `overlays` category, in sidebar order. */
export const overlaysSections: readonly ShowcaseSection[] = [
  {
    id: 'tooltip',
    title: 'Tooltip',
    description: 'Small dark labels with an optional shortcut, grouped by a provider.',
    category: 'overlays',
    icon: 'codicon:comment',
    covers: ['Tooltip'],
    component: TooltipSection,
  },
  {
    id: 'popover',
    title: 'Popover',
    description: 'Anchored floating panels on the raised popover surface, with an optional arrow.',
    category: 'overlays',
    icon: 'codicon:preview',
    covers: [
      'Popover',
      'PopoverClose',
      'PopoverDescription',
      'PopoverPopup',
      'PopoverTitle',
      'PopoverTrigger',
    ],
    component: PopoverSection,
  },
  {
    id: 'menu',
    title: 'Menu',
    description: 'Dropdown menus with icons, shortcuts, checks, radios and submenus.',
    category: 'overlays',
    icon: 'codicon:menu',
    covers: [
      'Menu',
      'MenuCheckboxItem',
      'MenuGroup',
      'MenuGroupLabel',
      'MenuHeader',
      'MenuItem',
      'MenuPopup',
      'MenuRadioGroup',
      'MenuRadioItem',
      'MenuSeparator',
      'MenuShortcut',
      'MenuSubmenuRoot',
      'MenuSubmenuTrigger',
      'MenuTrigger',
    ],
    component: MenuSection,
  },
  {
    id: 'context-menu',
    title: 'Context Menu',
    description: 'Right-click menus built from the same rows.',
    category: 'overlays',
    icon: 'codicon:list-unordered',
    covers: [
      'ContextMenu',
      'ContextMenuCheckboxItem',
      'ContextMenuGroup',
      'ContextMenuGroupLabel',
      'ContextMenuItem',
      'ContextMenuPopup',
      'ContextMenuRadioGroup',
      'ContextMenuRadioItem',
      'ContextMenuSeparator',
      'ContextMenuSubmenuRoot',
      'ContextMenuSubmenuTrigger',
      'ContextMenuTrigger',
    ],
    component: ContextMenuSection,
  },
  {
    id: 'dialog',
    title: 'Dialog',
    description: 'Modal sheets with a scrim, three widths and footer actions.',
    category: 'overlays',
    icon: 'codicon:window',
    covers: [
      'Dialog',
      'DialogBody',
      'DialogClose',
      'DialogDescription',
      'DialogFooter',
      'DialogPopup',
      'DialogTitle',
      'DialogTrigger',
    ],
    component: DialogSection,
  },
  {
    id: 'alert-dialog',
    title: 'Alert Dialog',
    description: 'Confirmations that require a decision, with a destructive tone.',
    category: 'overlays',
    icon: 'codicon:warning',
    covers: [
      'AlertDialog',
      'AlertDialogClose',
      'AlertDialogDescription',
      'AlertDialogFooter',
      'AlertDialogPopup',
      'AlertDialogTitle',
      'AlertDialogTrigger',
    ],
    component: AlertDialogSection,
  },
  {
    id: 'command-palette',
    title: 'Command Palette',
    description:
      'Quick open: fuzzy search with highlighted matches, groups, shortcuts and full keyboard control.',
    category: 'overlays',
    icon: 'codicon:symbol-event',
    covers: ['CommandPalette'],
    component: CommandPaletteSection,
  },
  {
    id: 'toast',
    title: 'Toast',
    description: 'Transient notifications stacked above the status bar.',
    category: 'overlays',
    icon: 'codicon:bell',
    covers: ['ToastViewport'],
    component: ToastSection,
  },
];

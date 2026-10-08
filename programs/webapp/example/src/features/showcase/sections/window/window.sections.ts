import type { ShowcaseSection } from '../../showcase.types';
import { AppShellSection } from './app-shell.section';
import { StatusBarSectionPage } from './status-bar.section';
import { TitleBarSection } from './title-bar.section';
import { TrafficLightsSection } from './traffic-lights.section';
import { WindowContextMenuSection } from './window-context-menu.section';

/** Showcase pages for the `window` category, in sidebar order. */
export const windowSections: readonly ShowcaseSection[] = [
  {
    id: 'title-bar',
    title: 'Title Bar',
    description:
      'A 38px unified titlebar: traffic lights, a command center and actions, with drag regions in the empty areas.',
    category: 'window',
    icon: 'codicon:window',
    covers: ['TitleBar', 'TitleBarCommandCenter'],
    component: TitleBarSection,
  },
  {
    id: 'traffic-lights',
    title: 'Traffic Lights',
    description: 'The window controls of every SLATECORE app, on every OS: 12px lights, 8px apart.',
    category: 'window',
    icon: 'codicon:ellipsis',
    covers: ['TrafficLights'],
    component: TrafficLightsSection,
  },
  {
    id: 'status-bar',
    title: 'Status Bar',
    description: 'A 24px status bar: static facts and clickable items, with one accent item.',
    category: 'window',
    icon: 'codicon:layout-statusbar',
    covers: ['StatusBar', 'StatusBarItem', 'StatusBarSection'],
    component: StatusBarSectionPage,
  },
  {
    id: 'app-shell',
    title: 'App Shell',
    description: 'The window grid with a resizable, collapsible sidebar and an optional inspector.',
    category: 'window',
    icon: 'codicon:layout',
    covers: ['AppShell'],
    component: AppShellSection,
  },
  {
    id: 'window-context-menu',
    title: 'Window Context Menu',
    description:
      'One right-click menu for the whole window that adapts to the titlebar, text boxes, content and status items.',
    category: 'window',
    icon: 'codicon:list-selection',
    covers: ['WindowContextMenu'],
    component: WindowContextMenuSection,
  },
];

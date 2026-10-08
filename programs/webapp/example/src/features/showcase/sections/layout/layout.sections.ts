import type { ShowcaseSection } from '../../showcase.types';
import { CardSectionPage } from './card.section';
import { PanelSectionPage } from './panel.section';
import { ScrollAreaSectionPage } from './scroll-area.section';
import { SidebarSectionPage } from './sidebar.section';

/** Showcase pages for the `layout` category, in sidebar order. */
export const layoutSections: readonly ShowcaseSection[] = [
  {
    id: 'sidebar',
    title: 'Sidebar',
    description:
      'The source list: sections, selectable rows with an inset pill that dims in background windows, header and footer slots.',
    category: 'layout',
    icon: 'codicon:layout-sidebar-left',
    covers: [
      'Sidebar',
      'SidebarContent',
      'SidebarFooter',
      'SidebarHeader',
      'SidebarItem',
      'SidebarSection',
      'SidebarTabs',
    ],
    component: SidebarSectionPage,
  },
  {
    id: 'panel',
    title: 'Panel',
    description:
      'Titled view regions for side panes and inspectors, with header actions that appear on hover.',
    category: 'layout',
    icon: 'codicon:layout-panel',
    covers: ['Panel', 'PanelBody', 'PanelFooter', 'PanelHeader'],
    component: PanelSectionPage,
  },
  {
    id: 'card',
    title: 'Card',
    description: 'Grouped content on a flat, hairlined surface.',
    category: 'layout',
    icon: 'codicon:preview',
    covers: ['Card', 'CardBody', 'CardFooter', 'CardHeader'],
    component: CardSectionPage,
  },
  {
    id: 'scroll-area',
    title: 'Scroll Area & Separator',
    description: 'Overlay scrollbars that appear on hover or while scrolling, and 1px separators.',
    category: 'layout',
    icon: 'codicon:list-flat',
    covers: ['ScrollArea', 'Separator'],
    component: ScrollAreaSectionPage,
  },
];

import type { ShowcaseSection } from '../../showcase.types';
import { BreadcrumbsSection } from './breadcrumbs.section';
import { TabsSection } from './tabs.section';
import { TreeSection } from './tree.section';

/** Showcase pages for the `navigation` category, in sidebar order. */
export const navigationSections: readonly ShowcaseSection[] = [
  {
    id: 'tabs',
    title: 'Tabs',
    description:
      'Underline tabs for panels and pill tabs for segmented views, with a sliding indicator.',
    category: 'navigation',
    icon: 'codicon:layout-panel',
    covers: ['Tab', 'Tabs', 'TabsIndicator', 'TabsList', 'TabsPanel'],
    component: TabsSection,
  },
  {
    id: 'tree',
    title: 'Tree',
    description: 'A WAI-ARIA tree view at file-explorer density.',
    category: 'navigation',
    icon: 'codicon:list-tree',
    covers: ['Tree', 'TreeItem'],
    component: TreeSection,
  },
  {
    id: 'breadcrumbs',
    title: 'Breadcrumbs',
    description: 'Where you are, one clickable segment at a time.',
    category: 'navigation',
    icon: 'codicon:chevron-right',
    covers: ['Breadcrumbs'],
    component: BreadcrumbsSection,
  },
];

import type { ShowcaseSection } from '../../showcase.types';
import { ButtonSection } from './button.section';
import { IconButtonSection } from './icon-button.section';
import { SegmentedControlSection } from './segmented-control.section';
import { ToggleButtonSection } from './toggle-button.section';
import { ToolbarSection } from './toolbar.section';

/** Showcase pages for the `actions` category, in sidebar order. */
export const actionsSections: readonly ShowcaseSection[] = [
  {
    id: 'button',
    title: 'Button',
    description:
      'Flat push buttons in five variants and four sizes, with a 1px border on every variant.',
    category: 'actions',
    icon: 'codicon:record-small',
    covers: ['Button'],
    component: ButtonSection,
  },
  {
    id: 'icon-button',
    title: 'Icon Button',
    description:
      'Quiet, square action-bar buttons; labelled, with an optional tooltip and toggled state.',
    category: 'actions',
    icon: 'codicon:gear',
    covers: ['IconButton'],
    component: IconButtonSection,
  },
  {
    id: 'toggle-button',
    title: 'Toggle Button',
    description: 'Two-state buttons, alone or in a roving-focus group.',
    category: 'actions',
    icon: 'codicon:bold',
    covers: ['ToggleButton', 'ToggleGroup'],
    component: ToggleButtonSection,
  },
  {
    id: 'segmented-control',
    title: 'Segmented Control',
    description: 'A single choice between a few equal options, with a sliding thumb.',
    category: 'actions',
    icon: 'codicon:split-horizontal',
    covers: ['SegmentedControl', 'SegmentedControlItem'],
    component: SegmentedControlSection,
  },
  {
    id: 'toolbar',
    title: 'Toolbar',
    description: 'Rows of controls with one tab stop and arrow-key navigation.',
    category: 'actions',
    icon: 'codicon:tools',
    covers: [
      'Toolbar',
      'ToolbarButton',
      'ToolbarGroup',
      'ToolbarSeparator',
      'ToolbarSpacer',
      'ToolbarTextButton',
    ],
    component: ToolbarSection,
  },
];

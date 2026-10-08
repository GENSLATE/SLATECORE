import type { ShowcaseSection } from '../../showcase.types';
import { ColorsSection } from './colors.section';
import { CursorsSection } from './cursors.section';
import { IconsSection } from './icons.section';
import { MotionSection } from './motion.section';
import { ProvidersSection } from './providers.section';
import { SpacingSection } from './spacing.section';
import { TypographySection } from './typography.section';

/** Showcase pages for the `foundations` category, in sidebar order. */
export const foundationsSections: readonly ShowcaseSection[] = [
  {
    id: 'colors',
    title: 'Colors',
    description:
      'Official Nord, mapped to semantic and chrome roles. Swatches follow the current theme.',
    category: 'foundations',
    icon: 'codicon:symbol-color',
    covers: [],
    component: ColorsSection,
  },
  {
    id: 'typography',
    title: 'Typography',
    description: 'Inter at a 13px base with optical tracking, JetBrains Mono for code.',
    category: 'foundations',
    icon: 'codicon:text-size',
    covers: [],
    component: TypographySection,
  },
  {
    id: 'spacing',
    title: 'Spacing & Elevation',
    description:
      'The 4px grid, radii, the single popover shadow and the fixed sizes of desktop chrome.',
    category: 'foundations',
    icon: 'codicon:symbol-ruler',
    covers: [],
    component: SpacingSection,
  },
  {
    id: 'motion',
    title: 'Motion',
    description:
      'Three easing curves, four durations and the motion-* utilities, with a reduced-motion switch.',
    category: 'foundations',
    icon: 'codicon:pulse',
    covers: [],
    component: MotionSection,
  },
  {
    id: 'cursors',
    title: 'Cursors',
    description:
      'A Nord cursor family that follows the theme and changes with what is under the pointer.',
    category: 'foundations',
    icon: 'codicon:inspect',
    covers: [],
    component: CursorsSection,
  },
  {
    id: 'icons',
    title: 'Icons',
    description: 'VS Code Codicons on a 16px grid, searchable and click to copy.',
    category: 'foundations',
    icon: 'codicon:symbol-misc',
    covers: ['Icon'],
    component: IconsSection,
  },
  {
    id: 'providers',
    title: 'Providers & Hooks',
    description:
      'The provider stack that themes and wires every component, and the hooks that read it.',
    category: 'foundations',
    icon: 'codicon:symbol-namespace',
    covers: [
      'DesignSystemProvider',
      'ThemeProvider',
      'PlatformProvider',
      'WindowStateProvider',
      'CursorProvider',
      'TooltipProvider',
      'ToastProvider',
      'ToastViewport',
    ],
    component: ProvidersSection,
  },
];

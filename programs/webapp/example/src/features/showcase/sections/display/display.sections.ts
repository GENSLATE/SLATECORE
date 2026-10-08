import type { ShowcaseSection } from '../../showcase.types';
import { AvatarSection } from './avatar.section';
import { CodeBlockSection } from './code-block.section';
import { ColorSwatchSection } from './color-swatch.section';
import { KbdSection } from './kbd.section';

/** Showcase pages for the `display` category, in sidebar order. */
export const displaySections: readonly ShowcaseSection[] = [
  {
    id: 'kbd',
    title: 'Keyboard Shortcut',
    description: 'Shortcuts as keycaps or inline text, with the right symbols for each platform.',
    category: 'display',
    icon: 'codicon:record-keys',
    covers: ['Kbd'],
    component: KbdSection,
  },
  {
    id: 'avatar',
    title: 'Avatar',
    description: 'People and workspaces, with initials and presence.',
    category: 'display',
    icon: 'codicon:account',
    covers: ['Avatar'],
    component: AvatarSection,
  },
  {
    id: 'code-block',
    title: 'Code Block',
    description: 'Readable, copyable source snippets.',
    category: 'display',
    icon: 'codicon:code',
    covers: ['CodeBlock'],
    component: CodeBlockSection,
  },
  {
    id: 'color-swatch',
    title: 'Color Swatch',
    description: 'Colour chips for token documentation and pickers.',
    category: 'display',
    icon: 'codicon:symbol-color',
    covers: ['ColorSwatch'],
    component: ColorSwatchSection,
  },
];

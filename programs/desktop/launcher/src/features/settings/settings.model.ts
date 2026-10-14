import type { CodiconRef } from '@genslate/design-system';

/** The Settings tool's sections, in order. */
export type SettingsSection = 'appearance' | 'behavior' | 'keybindings' | 'vault' | 'about';

export interface SectionInfo {
  readonly id: SettingsSection;
  readonly label: string;
  readonly icon: CodiconRef;
  /** One line under the section title. */
  readonly summary: string;
}

export const SETTINGS_SECTIONS: readonly SectionInfo[] = [
  {
    id: 'appearance',
    label: 'Appearance',
    icon: 'codicon:color-mode',
    summary: 'Theme, window size and what the status bar shows.',
  },
  {
    id: 'behavior',
    label: 'Behavior',
    icon: 'codicon:window',
    summary: 'When the launcher hides and stays on top.',
  },
  {
    id: 'keybindings',
    label: 'Keybindings',
    icon: 'codicon:record-keys',
    summary: 'Shortcuts, read from keybindings.toml.',
  },
  {
    id: 'vault',
    label: 'Vault',
    icon: 'codicon:lock',
    summary: 'Files in storage/vault, encrypted on this drive.',
  },
  {
    id: 'about',
    label: 'About this drive',
    icon: 'codicon:info',
    summary: 'The launcher, this drive, and what stays behind on a PC.',
  },
];

export function isSettingsSection(value: unknown): value is SettingsSection {
  return SETTINGS_SECTIONS.some((section) => section.id === value);
}

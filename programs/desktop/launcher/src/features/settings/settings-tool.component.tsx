import { ScrollArea, Tab, Tabs, TabsList, TabsPanel } from '@genslate/design-system';
import type { ReactNode } from 'react';

import { AboutSection } from './about-section.component';
import { AppearanceSection } from './appearance-section.component';
import { BehaviorSection } from './behavior-section.component';
import { KeybindingsSection } from './keybindings-section.component';
import { isSettingsSection, SETTINGS_SECTIONS, type SettingsSection } from './settings.model';
import { VaultSection } from './vault-section.component';

const CONTENT: Record<SettingsSection, () => ReactNode> = {
  appearance: () => <AppearanceSection />,
  behavior: () => <BehaviorSection />,
  keybindings: () => <KeybindingsSection />,
  vault: () => <VaultSection />,
  about: () => <AboutSection />,
};

export interface SettingsToolProps {
  readonly section: SettingsSection;
  readonly onSectionChange: (section: SettingsSection) => void;
}

/**
 * The Settings tool: five sections in a vertical tab list (the indicator slides between them),
 * each built from design-system parts. Changes go to settings.toml through the shell, which
 * echoes them back, so the tray menu, `/theme` and this tool always agree.
 */
export function SettingsTool({ section, onSectionChange }: SettingsToolProps) {
  return (
    <Tabs
      orientation="vertical"
      value={section}
      onValueChange={(next) => {
        if (isSettingsSection(next)) onSectionChange(next);
      }}
      className="h-full"
    >
      <TabsList
        aria-label="Settings sections"
        className="w-48 shrink-0 gap-0.5 bg-surface-sidebar px-2 py-3"
      >
        {SETTINGS_SECTIONS.map((info) => (
          <Tab
            key={info.id}
            value={info.id}
            icon={info.icon}
            className="h-8 justify-start gap-2 rounded-md px-2.5 text-sm hover:bg-fill-hover data-active:bg-fill-hover"
          >
            {info.label}
          </Tab>
        ))}
      </TabsList>
      {SETTINGS_SECTIONS.map((info) => (
        <TabsPanel key={info.id} value={info.id} className="min-h-0 min-w-0">
          <ScrollArea className="h-full" viewportClassName="px-7 py-5" scrollShadow>
            <div className="motion-fade-up flex max-w-xl flex-col gap-5">
              <header className="flex flex-col gap-0.5">
                <h2 className="font-semibold text-fg-strong text-lg">{info.label}</h2>
                <p className="text-fg-muted text-sm">{info.summary}</p>
              </header>
              {CONTENT[info.id]()}
            </div>
          </ScrollArea>
        </TabsPanel>
      ))}
    </Tabs>
  );
}

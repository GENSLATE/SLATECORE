import type { CodiconRef } from '@genslate/design-system';
import { cn, Icon, Tab, Tabs, TabsList, Tooltip } from '@genslate/design-system';

import type { Source, TabInfo } from '../../ipc/launcher.types';

export const SOURCE_ICON: Record<Source, CodiconRef> = {
  genslate: 'codicon:layers',
  portapps: 'codicon:archive',
  portableapps: 'codicon:package',
};

export const SOURCE_LABEL: Record<Source, string> = {
  genslate: 'GENSLATE',
  portapps: 'portapps.io',
  portableapps: 'PortableApps.com',
};

export interface SourceTabsProps {
  /** Already in launcher order: GENSLATE, portapps.io, PortableApps.com. */
  readonly tabs: readonly TabInfo[];
  readonly value: Source;
  readonly onChange: (source: Source) => void;
}

/**
 * GENSLATE · portapps.io · PortableApps.com as pill tabs with a sliding indicator. The active
 * tab spells out its name; the others show their icon and app count (full names in tooltips
 * and accessible names), so three sources fit the narrow panel. With one source, a quiet
 * heading replaces the tabs.
 */
export function SourceTabs({ tabs, value, onChange }: SourceTabsProps) {
  const only =
    tabs.length <= 1 ? (tabs[0] ?? { source: 'genslate', label: 'GENSLATE', count: 0 }) : undefined;
  if (only !== undefined) {
    return (
      <div className="flex h-11 shrink-0 items-center gap-2 px-3.5 text-fg-secondary">
        <Icon name={SOURCE_ICON[only.source]} size={14} />
        <h2 className="font-semibold text-2xs uppercase tracking-wider">
          {SOURCE_LABEL[only.source]} apps
        </h2>
        <span className="ml-auto text-fg-muted text-xs tabular-nums">{only.count}</span>
      </div>
    );
  }
  return (
    <Tabs
      variant="pill"
      value={value}
      onValueChange={(next) => {
        const source = tabs.find((tab) => tab.source === next)?.source;
        if (source !== undefined) onChange(source);
      }}
      className="shrink-0 px-2 pt-2 pb-1"
    >
      <TabsList aria-label="App sources" className="w-full">
        {tabs.map((tab) => {
          const active = tab.source === value;
          const label = SOURCE_LABEL[tab.source];
          return (
            <Tooltip key={tab.source} content={`${label} · ${tab.count} apps`} side="bottom">
              <Tab
                value={tab.source}
                icon={SOURCE_ICON[tab.source]}
                aria-label={`${label}, ${tab.count} apps`}
                className={cn('min-w-0 gap-1.5', active ? 'flex-auto' : 'flex-none px-2.5')}
              >
                {active ? <span className="truncate">{label}</span> : null}
                <span
                  className={cn(
                    'text-2xs tabular-nums',
                    active ? 'text-fg-muted' : 'text-fg-secondary',
                  )}
                >
                  {tab.count}
                </span>
              </Tab>
            </Tooltip>
          );
        })}
      </TabsList>
    </Tabs>
  );
}

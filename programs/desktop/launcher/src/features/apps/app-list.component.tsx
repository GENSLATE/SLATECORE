import { Button, EmptyState, ScrollArea } from '@genslate/design-system';

import type { AppEntry, Source } from '../../ipc/launcher.types';
import { AppGroupHeader } from './app-group-header.component';
import { AppRow } from './app-row.component';
import type { AppGroup } from './catalog.model';
import { SOURCE_ICON, SOURCE_LABEL } from './source-tabs.component';

/** Where each source's apps go on the drive (shown in the empty state). */
const SOURCE_PATH: Record<Source, string> = {
  genslate: 'programs/genslate',
  portapps: 'programs/portapps.io',
  portableapps: 'programs/portableapps.com',
};

export interface AppListProps {
  /** Grouped browsing, or `null` while showing search results. */
  readonly groups: readonly AppGroup[] | null;
  readonly results: readonly AppEntry[];
  readonly query: string;
  /** The tab being browsed (names the empty state). */
  readonly source: Source;
  readonly collapsed: ReadonlySet<string>;
  /** Position of the active option in the flattened list (a favourite also in its category is a distinct option). */
  readonly activeIndex: number;
  readonly launchingId: string | undefined;
  /** Changes whenever the list should replay its entrance (tab switch, show). */
  readonly animationKey: string;
  readonly listboxId: string;
  readonly onToggleGroup: (groupId: string) => void;
  readonly onActivate: (index: number) => void;
  readonly onLaunch: (app: AppEntry) => void;
  readonly onToggleFavorite: (app: AppEntry) => void;
  readonly onRescan: () => void;
}

/**
 * The listbox of the apps panel: grouped (Favorites, Recent, categories, Unavailable) while
 * browsing, ranked across every source while searching. Keyboard focus stays in the search
 * box; the active option is scrolled into view. An empty tab gets a designed empty state that
 * says where its apps go.
 */
export function AppList({
  groups,
  results,
  query,
  source,
  collapsed,
  activeIndex,
  launchingId,
  animationKey,
  listboxId,
  onToggleGroup,
  onActivate,
  onLaunch,
  onToggleFavorite,
  onRescan,
}: AppListProps) {
  const row = (app: AppEntry, index: number, showSource: boolean) => (
    <AppRow
      key={`${app.id}@${index}`}
      app={app}
      index={index}
      active={index === activeIndex}
      launching={app.id === launchingId}
      showSource={showSource}
      ref={index === activeIndex ? scrollIntoView : undefined}
      onHover={() => onActivate(index)}
      onLaunch={() => onLaunch(app)}
      onToggleFavorite={() => onToggleFavorite(app)}
    />
  );

  if (groups === null && results.length === 0) {
    return (
      <div className="motion-fade-up grid h-full place-items-center px-4">
        <EmptyState
          size="sm"
          icon="codicon:search"
          title={`No apps match “${query.trim()}”`}
          description="Try another name, or type / for commands."
        />
      </div>
    );
  }

  if (groups !== null && groups.length === 0) {
    return (
      <div key={source} className="motion-fade-up grid h-full place-items-center px-5">
        <EmptyState
          size="sm"
          icon={SOURCE_ICON[source]}
          title={`No ${SOURCE_LABEL[source]} apps yet`}
          description={
            <>
              Put apps in{' '}
              <span className="font-mono text-fg-secondary">{SOURCE_PATH[source]}/</span> on this
              drive and they appear here.
            </>
          }
          actions={
            <Button size="sm" variant="secondary" leadingIcon="codicon:refresh" onClick={onRescan}>
              Rescan
            </Button>
          }
        />
      </div>
    );
  }

  let index = 0;
  return (
    <ScrollArea className="h-full" viewportClassName="px-1.5 pb-1.5" scrollShadow>
      <div
        key={animationKey}
        id={listboxId}
        role="listbox"
        aria-label={groups === null ? 'Search results' : 'Apps'}
        className="flex flex-col gap-px pt-1"
      >
        {groups === null ? (
          <>
            <div className="flex h-7 items-center px-2 font-semibold text-2xs text-fg-muted uppercase tracking-wider">
              <span className="flex-1">Results</span>
              <span className="tabular-nums">{results.length}</span>
            </div>
            {results.map((app) => row(app, index++, true))}
          </>
        ) : (
          groups.map((group) => {
            const isCollapsed = collapsed.has(group.id);
            return (
              // biome-ignore lint/a11y/useSemanticElements: option groups inside a listbox must be role="group" (WAI-ARIA); a fieldset is not allowed there.
              <div
                key={group.id}
                role="group"
                aria-label={group.label}
                className="flex flex-col gap-px"
              >
                <AppGroupHeader
                  group={group}
                  collapsed={isCollapsed}
                  onToggle={() => onToggleGroup(group.id)}
                />
                {isCollapsed ? null : group.apps.map((app) => row(app, index++, false))}
              </div>
            );
          })
        )}
      </div>
    </ScrollArea>
  );
}

/** Keeps the active option visible as the keyboard moves through the list. */
function scrollIntoView(element: HTMLDivElement | null): void {
  element?.scrollIntoView({ block: 'nearest' });
}

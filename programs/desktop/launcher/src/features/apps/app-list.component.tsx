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
  /** Id of the search listbox, and the prefix of each group's listbox id. */
  readonly listboxId: string;
  readonly onToggleGroup: (groupId: string) => void;
  readonly onActivate: (index: number) => void;
  readonly onLaunch: (app: AppEntry) => void;
  readonly onToggleFavorite: (app: AppEntry) => void;
  readonly onRescan: () => void;
}

/**
 * The apps panel's options: grouped (Favorites, Recent, categories, Unavailable) while browsing,
 * one listbox per group, or ranked across every source in one listbox while searching. Keyboard
 * focus stays in the search box, which points at the active option; the active option is
 * scrolled into view. An empty tab gets a designed empty state that says where its apps go.
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
  const row = (key: string, app: AppEntry, index: number, showSource: boolean) => (
    <AppRow
      key={key}
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

  // Options are numbered across every group: the search box's arrows walk them all in order.
  let index = 0;
  return (
    <ScrollArea className="h-full" viewportClassName="px-1.5 pb-1.5" scrollShadow>
      {groups === null ? (
        <div key={animationKey} className="flex flex-col gap-px pt-1">
          <div className="flex h-7 items-center px-2 font-semibold text-2xs text-fg-muted uppercase tracking-wider">
            <span className="flex-1">Results</span>
            <span className="tabular-nums">{results.length}</span>
          </div>
          <div
            id={listboxId}
            role="listbox"
            aria-label="Search results"
            className="flex flex-col gap-px"
          >
            {results.map((app) => row(app.id, app, index++, true))}
          </div>
        </div>
      ) : (
        // Each group is a listbox of its own, labelled by its header: the collapse buttons sit
        // between the listboxes, never inside one (a listbox may hold options only).
        // biome-ignore lint/a11y/useSemanticElements: a fieldset groups form controls; this names the set of app listboxes.
        <div
          key={animationKey}
          role="group"
          aria-label="Apps"
          className="flex flex-col gap-px pt-1"
        >
          {groups.map((group, position) => {
            const isCollapsed = collapsed.has(group.id);
            const id = groupListboxId(listboxId, position);
            return (
              <div key={group.id} className="flex flex-col gap-px">
                <AppGroupHeader
                  group={group}
                  labelId={`${id}-label`}
                  controls={isCollapsed ? undefined : id}
                  collapsed={isCollapsed}
                  onToggle={() => onToggleGroup(group.id)}
                />
                {isCollapsed ? null : (
                  <div
                    id={id}
                    role="listbox"
                    aria-labelledby={`${id}-label`}
                    className="flex flex-col gap-px"
                  >
                    {group.apps.map((app) => row(`${group.id}:${app.id}`, app, index++, false))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </ScrollArea>
  );
}

/** DOM id of the listbox of the group at `position`. */
function groupListboxId(listboxId: string, position: number): string {
  return `${listboxId}-${position}`;
}

/**
 * The ids of the listboxes `AppList` shows (space separated, for the search box's
 * `aria-controls`), or `undefined` when it shows none (an empty state).
 */
export function appListboxIds(
  listboxId: string,
  groups: readonly AppGroup[] | null,
  results: readonly AppEntry[],
  collapsed: ReadonlySet<string>,
): string | undefined {
  if (groups === null) return results.length > 0 ? listboxId : undefined;
  const ids = groups.flatMap((group, position) =>
    collapsed.has(group.id) ? [] : [groupListboxId(listboxId, position)],
  );
  return ids.length > 0 ? ids.join(' ') : undefined;
}

/** Keeps the active option visible as the keyboard moves through the list. */
function scrollIntoView(element: HTMLDivElement | null): void {
  element?.scrollIntoView({ block: 'nearest' });
}

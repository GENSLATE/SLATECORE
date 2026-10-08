import { useReducedMotion, useToast } from '@genslate/design-system';
import { isCommandError } from '@genslate/tauri-bridge';
import { type KeyboardEvent, type RefObject, useEffect, useRef, useState } from 'react';

import { optionId } from '../features/apps/app-row.component';
import {
  flatten,
  groupApps,
  isLaunchable,
  orderTabs,
  searchApps,
  sourceOf,
} from '../features/apps/catalog.model';
import {
  paramsFor,
  parseSlash,
  runsOnChoose,
  type SlashSuggestion,
} from '../features/command-bar/slash.model';
import type { SettingsSection } from '../features/settings/settings.model';
import type { ToolId } from '../features/tools/tools.model';
import { isVaultError } from '../ipc/launcher.parse';
import type { ActionSpec, AppEntry, Source, StatusMode } from '../ipc/launcher.types';
import { useLauncher } from './launcher.context';
import {
  type CommandHost,
  type CommandId,
  type CommandParams,
  runCommand,
} from './launcher-commands.model';
import { collapseDelayMs } from './motion.util';

/** What the main panel shows. Only a tool widens the frame (`view.kind === 'tool'`). */
export type View =
  | { readonly kind: 'apps' }
  | { readonly kind: 'tool'; readonly id: ToolId }
  | { readonly kind: 'help' }
  | { readonly kind: 'details'; readonly id: string }
  | { readonly kind: 'args'; readonly id: string };

const APPS: View = { kind: 'apps' };
const LAUNCH_POP_MS = 420;

/** Where an app of each source lives on the drive. */
export const SOURCE_FOLDER: Readonly<Record<Source, string>> = {
  genslate: 'programs/genslate/',
  portapps: 'programs/portapps.io/',
  portableapps: 'programs/portableapps.com/',
};

/** All launcher UI state and behaviour; components stay presentational. */
export function useLauncherController() {
  const { backend, context, settings, list, pinned, stage, showCount, showView } = useLauncher();
  const toast = useToast();
  const reducedMotion = useReducedMotion();
  const inputRef = useRef<HTMLInputElement>(null);

  const [view, setViewState] = useState<View>(APPS);
  const [section, setSection] = useState<SettingsSection>('appearance');
  const [tab, setTab] = useState<Source>('genslate');
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => new Set(['unavailable']));
  const [launchingId, setLaunchingId] = useState<string | undefined>();
  const statusMode: StatusMode = settings.config.status.mode;
  const [slashHint, setSlashHint] = useState<string | undefined>();
  // The tool stays mounted while the frame shrinks, so it can fade out.
  const [shownTool, setShownTool] = useState<ToolId | undefined>();

  const tabs = orderTabs(list.tabs);
  const currentTab = tabs.some((info) => info.source === tab)
    ? tab
    : (tabs[0]?.source ?? 'genslate');
  const slash = parseSlash(query, context.actions, list.apps);
  const searching = !slash.active && query.trim() !== '';
  const groups = searching ? null : groupApps(list.apps, currentTab, list.recent);
  const results = searching ? searchApps(list.apps, query) : [];
  const navigable: readonly AppEntry[] = groups === null ? results : flatten(groups, collapsed);
  const count = slash.active ? slash.suggestions.length : navigable.length;
  const index = count === 0 ? -1 : Math.min(activeIndex, count - 1);
  const activeApp = slash.active ? undefined : navigable[index];
  const expanded = view.kind === 'tool';

  if (view.kind === 'tool' && shownTool !== view.id) setShownTool(view.id);

  const focusSearch = () => focusSoon(inputRef);

  function setView(next: View) {
    setViewState(next);
    if (next.kind === 'apps') focusSearch();
  }

  // Every show: fresh search, the requested view (the tray can ask for Help or Settings),
  // focus in the bar.
  useEffect(() => {
    if (showCount === 0) return;
    setQuery('');
    setActiveIndex(0);
    if (showView === 'settings') {
      setSection('appearance');
      setViewState({ kind: 'tool', id: 'settings' });
    } else setViewState(showView === 'help' ? { kind: 'help' } : APPS);
    focusSoon(inputRef);
  }, [showCount, showView]);

  // Every hide: back to the apps with an empty search, so the next show starts clean and no
  // tool (the vault form with a half-typed password, say) stays mounted while hidden.
  useEffect(() => {
    if (stage !== 'closed') return;
    setQuery('');
    setActiveIndex(0);
    setSlashHint(undefined);
    setViewState(APPS);
  }, [stage]);

  // Widen the shell's hit area at once; shrink it (and unmount the tool) once the frame has
  // finished shrinking (`--gs-duration-slow`, nothing to wait for under reduced motion).
  useEffect(() => {
    if (expanded) {
      backend.setExpanded(true).catch(reportError);
      return;
    }
    const timer = setTimeout(() => {
      setShownTool(undefined);
      backend.setExpanded(false).catch(reportError);
    }, collapseDelayMs(reducedMotion));
    return () => clearTimeout(timer);
  }, [backend, expanded, reducedMotion]);

  // Esc anywhere leaves a tool or sub-view (the search box handles its own Esc first).
  useEffect(() => {
    if (view.kind === 'apps') return;
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented) return;
      event.preventDefault();
      setViewState(APPS);
      focusSoon(inputRef);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [view.kind]);

  function notify(error: unknown) {
    const message =
      isCommandError(error) || isVaultError(error)
        ? error.message
        : error instanceof Error
          ? error.message
          : String(error);
    toast.add({ title: 'Something went wrong', description: message, type: 'error' });
  }

  function openTool(id: ToolId, settingsSection?: SettingsSection) {
    if (settingsSection !== undefined) setSection(settingsSection);
    setViewState({ kind: 'tool', id });
  }

  function toggleTool(id: ToolId) {
    if (view.kind === 'tool' && view.id === id) setView(APPS);
    else openTool(id, id === 'settings' && view.kind !== 'tool' ? 'appearance' : undefined);
  }

  function changeQuery(next: string) {
    setQuery(next);
    setActiveIndex(0);
    setSlashHint(undefined);
    // Searching shows the results; a command keeps the current view until it runs.
    if (next !== '' && !next.startsWith('/') && view.kind !== 'apps') setViewState(APPS);
  }

  function launch(app: AppEntry, args?: readonly string[]) {
    if (!isLaunchable(app)) {
      const key = app.id.slice(app.id.indexOf('/') + 1);
      toast.add({
        title:
          app.status === 'not-installed'
            ? `${app.name} isn't installed yet`
            : `${app.name} can't start`,
        description:
          app.status === 'not-installed'
            ? `Add it to ${SOURCE_FOLDER[sourceOf(app.id)]}${key}/ and it appears here.`
            : 'Its program or its app info is missing. Reinstall it, then rescan.',
        type: 'info',
      });
      return;
    }
    setLaunchingId(app.id);
    setTimeout(() => setLaunchingId(undefined), LAUNCH_POP_MS);
    backend.launch(app.id, args).then(() => {
      setQuery('');
      setViewState(APPS);
    }, notify);
  }

  function toggleFavorite(app: AppEntry) {
    backend.setOverride(app.id, { favorite: !app.favorite }).then(() => {
      toast.add({
        title: app.favorite
          ? `Removed ${app.name} from Favorites`
          : `Added ${app.name} to Favorites`,
        type: 'success',
      });
    }, notify);
  }

  // The launcher window's side of the shared command registry (the tray has its own host).
  const commands: CommandHost = {
    backend,
    apps: list.apps,
    pinned,
    launch: (app) => launch(app),
    show: (target) => {
      if (target.kind === 'tool') openTool(target.id, target.section);
      else if (target.kind === 'help') setViewState({ kind: 'help' });
      else setView(APPS);
    },
    inform: (message, tone) => toast.add({ title: message, type: tone }),
    fail: notify,
  };

  /** Runs a registry command from this window (slash bar, hotkeys, menus, rail). */
  function run(id: CommandId, params?: CommandParams) {
    runCommand(commands, id, params);
  }

  function togglePin() {
    run('pin');
  }

  function changeTab(source: Source) {
    setTab(source);
    setActiveIndex(0);
    setView(APPS);
  }

  function changeStatusMode(mode: StatusMode) {
    backend.setSetting('statusMode', mode).catch(notify);
  }

  function rescan() {
    run('rescan');
  }

  /** Runs a slash command: the shared registry, or the shell for ids it does not know. */
  function execute(action: ActionSpec, params: CommandParams) {
    setQuery('');
    setSlashHint(undefined);
    runCommand(commands, action.id, params);
  }

  function choose(suggestion: SlashSuggestion | undefined) {
    if (suggestion?.kind === 'action') {
      if (runsOnChoose(suggestion.action)) execute(suggestion.action, {});
      else changeQuery(`/${suggestion.action.id} `);
      return;
    }
    if (suggestion?.kind === 'value') {
      execute(suggestion.action, { [suggestion.param.name]: suggestion.value });
      return;
    }
    if (slash.action === undefined) return;
    const parsed = paramsFor(slash.action, slash.argument);
    if ('error' in parsed) setSlashHint(parsed.error);
    else execute(slash.action, parsed.params);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    const move = (delta: number) => {
      event.preventDefault();
      if (count > 0) setActiveIndex((index + delta + count) % count);
    };
    switch (event.key) {
      case 'ArrowDown':
        return move(1);
      case 'ArrowUp':
        return move(-1);
      case 'Enter':
        event.preventDefault();
        if (slash.active) choose(slash.suggestions[index]);
        else if (activeApp !== undefined) launch(activeApp);
        return;
      case 'Tab':
        if (slash.active && slash.suggestions[index]?.kind === 'action') {
          event.preventDefault();
          const action = slash.suggestions[index].action;
          changeQuery(`/${action.id}${action.params.length > 0 ? ' ' : ''}`);
        }
        return;
      case 'Escape':
        event.preventDefault();
        if (query !== '') changeQuery('');
        else if (view.kind !== 'apps') setView(APPS);
        else backend.hide().catch(notify);
        return;
      case 'ContextMenu':
      case 'F10':
        if (event.key === 'F10' && !event.shiftKey) return;
        if (activeApp !== undefined) openContextMenu(optionId(index), event);
        return;
      default:
    }
  }

  return {
    inputRef,
    view,
    setView,
    section,
    setSection,
    shownTool,
    openTool,
    toggleTool,
    tabs,
    tab: currentTab,
    changeTab,
    query,
    changeQuery,
    slash,
    slashHint,
    activeIndex: index,
    setActiveIndex,
    activeApp,
    groups,
    results,
    collapsed,
    toggleGroup: (id: string) =>
      setCollapsed((current) => {
        const next = new Set(current);
        if (!next.delete(id)) next.add(id);
        return next;
      }),
    launchingId,
    expanded,
    statusMode,
    changeStatusMode,
    launch,
    toggleFavorite,
    togglePin,
    rescan,
    run,
    choose,
    onKeyDown,
    notify,
  };
}

/** Opens the list's context menu on a row from the keyboard (Shift+F10 / Menu key). */
function openContextMenu(rowId: string, event: KeyboardEvent) {
  event.preventDefault();
  const row = document.getElementById(rowId);
  if (row === null) return;
  const rect = row.getBoundingClientRect();
  row.dispatchEvent(
    new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: rect.left + 48,
      clientY: rect.top + rect.height / 2,
    }),
  );
}

/** Focuses the search box after the next paint (once the view it lives in is shown). */
function focusSoon(input: RefObject<HTMLInputElement | null>): void {
  requestAnimationFrame(() => input.current?.focus());
}

function reportError(error: unknown) {
  console.warn('launcher', error);
}

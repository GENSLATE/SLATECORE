import { AppContextMenu } from '../features/apps/app-context-menu.component';
import { AppDetails, RunWithArgs } from '../features/apps/app-details.component';
import { AppList, appListboxIds } from '../features/apps/app-list.component';
import { optionId } from '../features/apps/app-row.component';
import { SourceTabs } from '../features/apps/source-tabs.component';
import { CommandBar } from '../features/command-bar/command-bar.component';
import { suggestionId } from '../features/command-bar/slash-menu.component';
import { LauncherContextMenu } from '../features/frame/launcher-context-menu.component';
import { LauncherFrame } from '../features/frame/launcher-frame.component';
import { LauncherTitleBar } from '../features/frame/launcher-titlebar.component';
import { DocumentsRail } from '../features/rail/documents-rail.component';
import { ToolButtons } from '../features/rail/tool-buttons.component';
import { LauncherStatusBar } from '../features/status/launcher-status-bar.component';
import { HelpView } from '../features/tools/help-view.component';
import { ToolView } from '../features/tools/tool-view.component';
import { useLauncher } from './launcher.context';
import { useLauncherController } from './use-launcher-controller.hook';
import { useLauncherHotkeys } from './use-launcher-hotkeys.hook';

const LISTBOX_ID = 'launcher-apps';
const SLASH_ID = 'launcher-slash';

/**
 * The launcher: a flat frame with the documents rail on the left, the apps panel (tabs, list,
 * sub-views) or a tool on the right, the command bar under it and the status bar along the
 * bottom. A tool widens the frame; the list and the tool cross-fade.
 */
export function App() {
  const { backend, context, settings, list, pinned, vault, showCount } = useLauncher();
  const c = useLauncherController();
  const keys = settings.keybindings.launcher;
  useLauncherHotkeys(keys, {
    focusSearch: () => c.inputRef.current?.focus(),
    toggleSettings: () => c.toggleTool('settings'),
    togglePin: c.togglePin,
    toggleFavorite: () => {
      if (c.activeApp !== undefined) c.toggleFavorite(c.activeApp);
    },
    selectTab: c.changeTab,
  });

  const setPopupOpen = (open: boolean) => backend.setPopupOpen(open).catch(c.notify);
  const find = (id: string) => list.apps.find((app) => app.id === id);
  const slashIndexId = c.activeIndex >= 0 ? suggestionId(SLASH_ID, c.activeIndex) : undefined;
  const activeDescendant = c.slash.active ? slashIndexId : c.activeApp && optionId(c.activeIndex);
  const subview = c.view.kind === 'help' || c.view.kind === 'details' || c.view.kind === 'args';
  const openTool = c.view.kind === 'tool' ? c.view.id : undefined;
  const detailsApp =
    c.view.kind === 'details' || c.view.kind === 'args' ? find(c.view.id) : undefined;
  const back = () => c.setView({ kind: 'apps' });
  // What the search box drives: the suggestions while a command is typed, else the app list
  // when it is on screen (not under a tool or a sub-view, not an empty state).
  const controls = c.slash.active
    ? c.slash.suggestions.length > 0
      ? SLASH_ID
      : undefined
    : subview || c.expanded
      ? undefined
      : appListboxIds(LISTBOX_ID, c.groups, c.results, c.collapsed);
  const openSettingsFile = () => c.run('config', { file: 'settings' });

  const appsPane = (
    <div
      data-layer="apps"
      data-state={c.expanded ? 'hidden' : 'shown'}
      aria-hidden={c.expanded || undefined}
      inert={c.expanded}
      className="launcher-layer absolute inset-y-0 right-0 flex w-[calc(var(--launcher-normal)-var(--spacing-launcher-rail))] flex-col"
    >
      {c.view.kind === 'help' ? (
        <HelpView actions={context.actions} keybindings={settings.keybindings} onBack={back} />
      ) : detailsApp !== undefined && c.view.kind === 'details' ? (
        <AppDetails
          app={detailsApp}
          onBack={back}
          onLaunch={() => c.launch(detailsApp)}
          onOpenFolder={() => backend.openAppFolder(detailsApp.id).catch(c.notify)}
          onToggleFavorite={() => c.toggleFavorite(detailsApp)}
        />
      ) : detailsApp !== undefined && c.view.kind === 'args' ? (
        <RunWithArgs app={detailsApp} onBack={back} onRun={(args) => c.launch(detailsApp, args)} />
      ) : (
        <>
          <SourceTabs tabs={c.tabs} value={c.tab} onChange={c.changeTab} />
          <div className="min-h-0 flex-1">
            <AppContextMenu
              apps={list.apps}
              favoriteShortcut={keys.toggleFavorite}
              onOpenChange={setPopupOpen}
              actions={{
                launch: (app) => c.launch(app),
                runWithArgs: (app) => c.setView({ kind: 'args', id: app.id }),
                toggleFavorite: c.toggleFavorite,
                openFolder: (app) => backend.openAppFolder(app.id).catch(c.notify),
                hide: (app) => backend.setOverride(app.id, { hidden: true }).catch(c.notify),
                properties: (app) => c.setView({ kind: 'details', id: app.id }),
              }}
            >
              <AppList
                groups={c.groups}
                results={c.results}
                query={c.query}
                source={c.tab}
                collapsed={c.collapsed}
                activeIndex={c.slash.active ? -1 : c.activeIndex}
                launchingId={c.launchingId}
                animationKey={`${c.tab}:${showCount}:${c.groups === null ? 'search' : 'browse'}`}
                listboxId={LISTBOX_ID}
                onToggleGroup={c.toggleGroup}
                onActivate={c.setActiveIndex}
                onLaunch={c.launch}
                onToggleFavorite={c.toggleFavorite}
                onRescan={c.rescan}
              />
            </AppContextMenu>
          </div>
        </>
      )}
    </div>
  );

  const toolPane =
    c.shownTool === undefined ? null : (
      <div
        data-layer="tool"
        data-state={c.expanded ? 'shown' : 'hidden'}
        aria-hidden={!c.expanded || undefined}
        inert={!c.expanded}
        className="launcher-layer absolute inset-y-0 right-0 w-[calc(var(--launcher-expanded)-var(--spacing-launcher-rail))]"
      >
        <ToolView
          tool={c.shownTool}
          section={c.section}
          onSectionChange={c.setSection}
          onClose={back}
        />
      </div>
    );

  return (
    <LauncherContextMenu
      pinned={pinned}
      pinShortcut={keys.togglePin}
      onTogglePin={c.togglePin}
      settingsOpen={openTool === 'settings'}
      settingsShortcut={keys.toggleTools}
      onToggleSettings={() => c.toggleTool('settings')}
      onShowHelp={() => c.run('help')}
      onHide={() => backend.hide().catch(c.notify)}
      statusMode={c.statusMode}
      onStatusModeChange={c.changeStatusMode}
      onOpenSettingsFile={openSettingsFile}
      onOpenFolder={(folder) => c.run('folder', { folder })}
      onOpenChange={setPopupOpen}
      onError={c.notify}
    >
      <LauncherFrame
        expanded={c.expanded}
        size={settings.config.appearance.size}
        titleBar={
          <LauncherTitleBar
            mode={context.mode}
            pinned={pinned}
            pinShortcut={keys.togglePin}
            onTogglePin={c.togglePin}
            onHide={() => backend.hide().catch(c.notify)}
          />
        }
        rail={
          <DocumentsRail
            profile={context.profile}
            suiteName={context.suiteName}
            vault={vault.state}
            onOpenFolder={(folder) => c.run('folder', { folder })}
            onOpenVault={() => c.run('vault')}
          />
        }
        railFooter={
          <ToolButtons
            active={openTool}
            settingsShortcut={keys.toggleTools}
            onToggle={c.toggleTool}
          />
        }
        main={
          <>
            {appsPane}
            {toolPane}
          </>
        }
        commandBar={
          <CommandBar
            inputRef={c.inputRef}
            value={c.query}
            onChange={c.changeQuery}
            onKeyDown={c.onKeyDown}
            controls={controls}
            activeDescendant={activeDescendant}
            focusShortcut={keys.focusSearch}
            aiOpen={openTool === 'ai'}
            onAsk={() => c.toggleTool('ai')}
            slash={
              c.slash.active
                ? {
                    id: SLASH_ID,
                    suggestions: c.slash.suggestions,
                    activeIndex: c.activeIndex,
                    hint:
                      c.slashHint ??
                      (c.slash.action?.id === 'ask'
                        ? 'The AI assistant is coming soon. Press Enter to see what it will do.'
                        : undefined),
                    onHover: c.setActiveIndex,
                    onChoose: (index) => c.choose(c.slash.suggestions[index]),
                  }
                : null
            }
          />
        }
        statusBar={
          <LauncherStatusBar
            mode={c.statusMode}
            onModeChange={c.changeStatusMode}
            onOpenSettingsFile={openSettingsFile}
          />
        }
      />
    </LauncherContextMenu>
  );
}

import {
  ContextMenuItem,
  WindowContextMenu,
  type WindowContextTarget,
} from '@genslate/design-system';
import type { ReactNode } from 'react';

import type { SharedFolder, StatusMode } from '../../ipc/launcher.types';

const FOLDERS: readonly SharedFolder[] = [
  'desktop',
  'documents',
  'downloads',
  'music',
  'pictures',
  'videos',
  'storage',
];

const isFolder = (value: string | null | undefined): value is SharedFolder =>
  FOLDERS.some((folder) => folder === value);

export interface LauncherContextMenuProps {
  readonly pinned: boolean;
  readonly pinShortcut: string;
  readonly onTogglePin: () => void;
  readonly settingsOpen: boolean;
  readonly settingsShortcut: string;
  readonly onToggleSettings: () => void;
  readonly onShowHelp: () => void;
  readonly onHide: () => void;
  readonly statusMode: StatusMode;
  readonly onStatusModeChange: (mode: StatusMode) => void;
  readonly onOpenSettingsFile: () => void;
  readonly onOpenFolder: (folder: SharedFolder) => void;
  /** Tells the shell a popup may extend beyond the frame (no click-through while open). */
  readonly onOpenChange: (open: boolean) => void;
  readonly onError: (error: unknown) => void;
  readonly children: ReactNode;
}

/**
 * The launcher's window menu: the shared design-system menu (Theme, Edit commands in the search
 * box, Copy on status readings) plus launcher rows: Pin on Top, Settings and Help on the
 * titlebar, the status reading and the settings file on the status bar, Open on a rail folder.
 * The app list keeps its own per-app menu. Hiding sends the window back to the tray.
 */
export function LauncherContextMenu({
  pinned,
  pinShortcut,
  onTogglePin,
  settingsOpen,
  settingsShortcut,
  onToggleSettings,
  onShowHelp,
  onHide,
  statusMode,
  onStatusModeChange,
  onOpenSettingsFile,
  onOpenFolder,
  onOpenChange,
  onError,
  children,
}: LauncherContextMenuProps) {
  const items = (target: WindowContextTarget) => {
    if (target.kind === 'titlebar') {
      return (
        <>
          <ContextMenuItem
            icon={pinned ? 'codicon:pinned' : 'codicon:pin'}
            shortcut={pinShortcut}
            onClick={onTogglePin}
          >
            {pinned ? 'Unpin' : 'Pin on Top'}
          </ContextMenuItem>
          <ContextMenuItem
            icon="codicon:settings-gear"
            shortcut={settingsShortcut}
            onClick={onToggleSettings}
          >
            {settingsOpen ? 'Close Settings' : 'Settings'}
          </ContextMenuItem>
          <ContextMenuItem icon="codicon:question" onClick={onShowHelp}>
            Help
          </ContextMenuItem>
        </>
      );
    }
    if (target.kind === 'statusbar') {
      const hasTemps = target.element.closest('[data-has-temps]') !== null;
      return (
        <>
          {hasTemps ? (
            <ContextMenuItem
              icon="codicon:arrow-swap"
              onClick={() => onStatusModeChange(statusMode === 'temps' ? 'usage' : 'temps')}
            >
              {statusMode === 'temps' ? 'Show Usage' : 'Show Temperatures'}
            </ContextMenuItem>
          ) : null}
          <ContextMenuItem icon="codicon:go-to-file" onClick={onOpenSettingsFile}>
            Open settings.toml
          </ContextMenuItem>
        </>
      );
    }
    const folder = target.element.closest('[data-folder]')?.getAttribute('data-folder');
    if (target.zone === 'rail' && isFolder(folder)) {
      return (
        <ContextMenuItem icon="codicon:folder-opened" onClick={() => onOpenFolder(folder)}>
          {folder === 'storage'
            ? 'Open Storage Folder'
            : `Open ${folder.charAt(0).toUpperCase()}${folder.slice(1)}`}
        </ContextMenuItem>
      );
    }
    return null;
  };

  return (
    <WindowContextMenu
      allowNativeMenu={import.meta.env.DEV}
      onClose={onHide}
      labels={{ close: 'Hide to Tray' }}
      items={items}
      onOpenChange={onOpenChange}
      onError={onError}
    >
      {children}
    </WindowContextMenu>
  );
}

import {
  type CodiconRef,
  MenuGroup,
  MenuGroupLabel,
  MenuItem,
  MenuPopup,
  MenuRadioGroup,
  MenuRadioItem,
  MenuSeparator,
  MenuSubmenuRoot,
  MenuSubmenuTrigger,
} from '@genslate/design-system';

import { useLauncher } from '../../app/launcher.context';
import type { AppEntry, SharedFolder, SizePreset, ThemeSetting } from '../../ipc/launcher.types';
import { AppIcon } from '../apps/app-icon.component';
import { isLaunchable } from '../apps/catalog.model';
import type { TrayRun } from './use-tray-commands.hook';

const THEMES: readonly { value: ThemeSetting; label: string }[] = [
  { value: 'polar-night', label: 'Polar Night' },
  { value: 'snow-storm', label: 'Snow Storm' },
  { value: 'system', label: 'Match System' },
];

const SIZES: readonly { value: SizePreset; label: string }[] = [
  { value: 's', label: 'Small' },
  { value: 'm', label: 'Medium' },
  { value: 'l', label: 'Large' },
];

const FOLDERS: readonly { folder: SharedFolder; label: string; icon: CodiconRef }[] = [
  { folder: 'desktop', label: 'Desktop', icon: 'codicon:vm' },
  { folder: 'documents', label: 'Documents', icon: 'codicon:file-text' },
  { folder: 'downloads', label: 'Downloads', icon: 'codicon:desktop-download' },
  { folder: 'music', label: 'Music', icon: 'codicon:music' },
  { folder: 'pictures', label: 'Pictures', icon: 'codicon:file-media' },
  { folder: 'videos', label: 'Videos', icon: 'codicon:device-camera-video' },
];

export interface TrayAppsSubmenuProps {
  readonly label: string;
  readonly icon: CodiconRef;
  readonly apps: readonly AppEntry[];
  /** Shown (disabled) when `apps` is empty. */
  readonly empty: string;
  readonly run: TrayRun;
}

/** Recent or Favorites: one row per app with its icon; unavailable apps are disabled. */
export function TrayAppsSubmenu({ label, icon, apps, empty, run }: TrayAppsSubmenuProps) {
  return (
    <MenuSubmenuRoot>
      <MenuSubmenuTrigger icon={icon}>{label}</MenuSubmenuTrigger>
      <MenuPopup className="min-w-52">
        {apps.length === 0 ? <MenuItem disabled>{empty}</MenuItem> : null}
        {apps.map((app) => (
          <MenuItem
            key={app.id}
            media={<AppIcon app={app} size="sm" />}
            disabled={!isLaunchable(app)}
            onClick={() => run('open', { app: app.id })}
          >
            {app.name}
          </MenuItem>
        ))}
      </MenuPopup>
    </MenuSubmenuRoot>
  );
}

/** The portable folders (`storage/users/shared/*`) and the storage root. */
export function TrayFoldersSubmenu({ run }: { readonly run: TrayRun }) {
  const open = (folder: SharedFolder) => run('folder', { folder });
  return (
    <MenuSubmenuRoot>
      <MenuSubmenuTrigger icon="codicon:folder">Folders</MenuSubmenuTrigger>
      <MenuPopup>
        {FOLDERS.map(({ folder, label, icon }) => (
          <MenuItem key={folder} icon={icon} onClick={() => open(folder)}>
            {label}
          </MenuItem>
        ))}
        <MenuSeparator />
        <MenuItem icon="codicon:database" onClick={() => open('storage')}>
          Storage Folder
        </MenuItem>
      </MenuPopup>
    </MenuSubmenuRoot>
  );
}

/** Theme and window size, written to config.toml (the shell applies them live). */
export function TrayAppearanceSubmenu({ run }: { readonly run: TrayRun }) {
  const { settings } = useLauncher();
  const { theme, size } = settings.config.appearance;
  return (
    <MenuSubmenuRoot>
      <MenuSubmenuTrigger icon="codicon:color-mode">Appearance</MenuSubmenuTrigger>
      <MenuPopup>
        <MenuGroup>
          <MenuRadioGroup
            value={theme}
            onValueChange={(value) => {
              if (typeof value === 'string') run('theme', { mode: value });
            }}
          >
            <MenuGroupLabel inset>Theme</MenuGroupLabel>
            {THEMES.map(({ value, label }) => (
              <MenuRadioItem key={value} value={value} closeOnClick>
                {label}
              </MenuRadioItem>
            ))}
          </MenuRadioGroup>
        </MenuGroup>
        <MenuSeparator />
        <MenuGroup>
          <MenuRadioGroup
            value={size}
            onValueChange={(value) => {
              if (typeof value === 'string') run('size', { preset: value });
            }}
          >
            <MenuGroupLabel inset>Window Size</MenuGroupLabel>
            {SIZES.map(({ value, label }) => (
              <MenuRadioItem key={value} value={value} closeOnClick>
                {label}
              </MenuRadioItem>
            ))}
          </MenuRadioGroup>
        </MenuGroup>
      </MenuPopup>
    </MenuSubmenuRoot>
  );
}

/** The Settings tool, the launcher's own files and a rescan. */
export function TraySettingsSubmenu({ run }: { readonly run: TrayRun }) {
  return (
    <MenuSubmenuRoot>
      <MenuSubmenuTrigger icon="codicon:settings-gear">Settings</MenuSubmenuTrigger>
      <MenuPopup className="min-w-52">
        <MenuItem icon="codicon:settings-gear" onClick={() => run('settings')}>
          Open Settings…
        </MenuItem>
        <MenuSeparator />
        <MenuItem icon="codicon:go-to-file" onClick={() => run('config', { file: 'settings' })}>
          Edit settings.toml
        </MenuItem>
        <MenuItem icon="codicon:record-keys" onClick={() => run('config', { file: 'keybindings' })}>
          Edit keybindings.toml
        </MenuItem>
        <MenuItem icon="codicon:output" onClick={() => run('config', { file: 'logs' })}>
          Open Logs
        </MenuItem>
        <MenuSeparator />
        <MenuItem icon="codicon:refresh" onClick={() => run('rescan')}>
          Rescan Apps
        </MenuItem>
      </MenuPopup>
    </MenuSubmenuRoot>
  );
}

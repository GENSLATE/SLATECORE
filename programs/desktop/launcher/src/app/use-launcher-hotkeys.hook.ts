import { useHotkey } from '@genslate/design-system';

import type { LauncherKeys, Source } from '../ipc/launcher.types';

export interface LauncherHotkeyHandlers {
  readonly focusSearch: () => void;
  readonly toggleSettings: () => void;
  readonly togglePin: () => void;
  readonly toggleFavorite: () => void;
  readonly selectTab: (source: Source) => void;
}

/** In-launcher shortcuts from keybindings.toml (`""` disables one). They work inside inputs. */
export function useLauncherHotkeys(keys: LauncherKeys, handlers: LauncherHotkeyHandlers): void {
  const options = (shortcut: string) => ({ allowInInputs: true, enabled: shortcut !== '' });
  // `useHotkey` needs a parsable shortcut even when disabled.
  const or = (shortcut: string) => shortcut || 'f24';
  useHotkey(or(keys.focusSearch), handlers.focusSearch, options(keys.focusSearch));
  useHotkey(or(keys.toggleTools), handlers.toggleSettings, options(keys.toggleTools));
  useHotkey(or(keys.togglePin), handlers.togglePin, options(keys.togglePin));
  useHotkey(or(keys.toggleFavorite), handlers.toggleFavorite, options(keys.toggleFavorite));
  useHotkey(or(keys.tabGenslate), () => handlers.selectTab('genslate'), options(keys.tabGenslate));
  useHotkey(or(keys.tabPortapps), () => handlers.selectTab('portapps'), options(keys.tabPortapps));
  useHotkey(
    or(keys.tabPortableapps),
    () => handlers.selectTab('portableapps'),
    options(keys.tabPortableapps),
  );
}

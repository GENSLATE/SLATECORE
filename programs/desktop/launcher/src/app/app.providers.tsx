import {
  DesignSystemProvider,
  ToastProvider,
  ToastViewport,
  TooltipProvider,
} from '@genslate/design-system';
import { detectPlatform, useSystemTheme } from '@genslate/tauri-bridge';
import { type ReactNode, useState } from 'react';

import { useLauncher } from './launcher.context';

/**
 * Platform and theme for the whole launcher. The theme comes from settings.toml (never from
 * browser storage): editing the file, the Settings tool or `/theme` update it live.
 */
export function AppProviders({ children }: { children: ReactNode }) {
  const [platform] = useState(detectPlatform);
  const systemScheme = useSystemTheme();
  const { backend, settings } = useLauncher();

  return (
    <DesignSystemProvider
      platform={platform}
      systemScheme={systemScheme}
      theme={settings.config.appearance.theme}
      // The context menu's Theme submenu writes settings.toml; the shell echoes it back.
      onThemeChange={(theme) => {
        backend
          .setSetting('theme', theme)
          .catch((error: unknown) => console.warn('launcher', error));
      }}
      storageKey={null}
      windowState={{ isFocused: true, isMaximized: false, isFullscreen: false }}
    >
      <TooltipProvider>
        <ToastProvider limit={2} timeout={3500}>
          {children}
          <ToastViewport />
        </ToastProvider>
      </TooltipProvider>
    </DesignSystemProvider>
  );
}

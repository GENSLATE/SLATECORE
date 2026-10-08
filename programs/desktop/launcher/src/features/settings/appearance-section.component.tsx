import { SegmentedControl, SegmentedControlItem } from '@genslate/design-system';

import { useLauncher } from '../../app/launcher.context';
import type { SizePreset, StatusMode, ThemeSetting } from '../../ipc/launcher.types';
import { SettingsGroup, SettingsRow } from './settings-row.component';
import { useReport } from './use-report.hook';

/** Theme, window size and the status bar readings, written to settings.toml. */
export function AppearanceSection() {
  const { backend, settings } = useLauncher();
  const report = useReport();
  const { theme, size } = settings.config.appearance;
  const write = (key: 'theme' | 'size' | 'statusMode', value: string) =>
    backend.setSetting(key, value).catch(report);

  return (
    <>
      <SettingsGroup title="Theme">
        <SettingsRow
          labelId="settings-theme"
          label="Colour theme"
          description="Official Nord: Polar Night is dark, Snow Storm is light."
          control={
            <SegmentedControl<ThemeSetting>
              aria-labelledby="settings-theme"
              value={theme}
              onValueChange={(value) => write('theme', value)}
            >
              <SegmentedControlItem value="polar-night" icon="codicon:color-mode">
                Polar Night
              </SegmentedControlItem>
              <SegmentedControlItem value="snow-storm" icon="codicon:symbol-color">
                Snow Storm
              </SegmentedControlItem>
              <SegmentedControlItem value="system" icon="codicon:vm">
                System
              </SegmentedControlItem>
            </SegmentedControl>
          }
        />
      </SettingsGroup>
      <SettingsGroup title="Window">
        <SettingsRow
          labelId="settings-size"
          label="Size"
          description="Height of the launcher in the corner of the screen: 560, 640 or 740 px."
          control={
            <SegmentedControl<SizePreset>
              aria-labelledby="settings-size"
              value={size}
              onValueChange={(value) => write('size', value)}
            >
              <SegmentedControlItem value="s">Small</SegmentedControlItem>
              <SegmentedControlItem value="m">Medium</SegmentedControlItem>
              <SegmentedControlItem value="l">Large</SegmentedControlItem>
            </SegmentedControl>
          }
        />
        <SettingsRow
          labelId="settings-status"
          label="Status bar"
          description="Readings next to the drive: CPU and GPU temperatures, or usage and network."
          control={
            <SegmentedControl<StatusMode>
              aria-labelledby="settings-status"
              value={settings.config.status.mode}
              onValueChange={(value) => write('statusMode', value)}
            >
              <SegmentedControlItem value="temps">Temperatures</SegmentedControlItem>
              <SegmentedControlItem value="usage">Usage</SegmentedControlItem>
            </SegmentedControl>
          }
        />
      </SettingsGroup>
    </>
  );
}

import { SegmentedControl, SegmentedControlItem } from '@genslate/design-system';

import { useLauncher } from '../../app/launcher.context';
import type { SizePreset, StatusMode } from '../../ipc/launcher.types';
import { SettingsGroup, SettingsRow } from './settings-row.component';
import { ThemePicker } from './theme-picker.component';
import { useReport } from './use-report.hook';

/**
 * One fixed width for both segmented controls: they line up, and their segments land on whole
 * pixels (240 − 2 border − 4 padding = 234, split in 2 or 3).
 */
const SEGMENTS = 'w-60';

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
          stacked
          labelId="settings-theme"
          label="Color theme"
          description="Official Nord: Polar Night is dark, Snow Storm is light, System follows Windows."
          control={
            <ThemePicker
              labelledBy="settings-theme"
              value={theme}
              onValueChange={(value) => write('theme', value)}
            />
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
              className={SEGMENTS}
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
          description="Next to the drive: CPU and GPU temperatures, or usage and network speed."
          control={
            <SegmentedControl<StatusMode>
              aria-labelledby="settings-status"
              className={SEGMENTS}
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

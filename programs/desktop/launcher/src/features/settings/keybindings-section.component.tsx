import { Button, Kbd } from '@genslate/design-system';

import { useLauncher } from '../../app/launcher.context';
import { SettingsGroup, SettingsRow } from './settings-row.component';
import { useReport } from './use-report.hook';

function Keys({ shortcut }: { shortcut: string }) {
  return shortcut === '' ? (
    <span className="text-fg-muted text-xs">Off</span>
  ) : (
    <Kbd shortcut={shortcut} size="sm" />
  );
}

/** The shortcuts in use, from keybindings.toml (an empty value switches one off). */
export function KeybindingsSection() {
  const { backend, settings } = useLauncher();
  const report = useReport();
  const { global, launcher } = settings.keybindings;
  const rows: readonly (readonly [string, string])[] = [
    ['Search apps', launcher.focusSearch],
    ['Open or close Settings', launcher.toggleTools],
    ['Pin on top', launcher.togglePin],
    ['Add to or remove from Favorites', launcher.toggleFavorite],
    ['GENSLATE tab', launcher.tabGenslate],
    ['portapps.io tab', launcher.tabPortapps],
    ['PortableApps.com tab', launcher.tabPortableapps],
  ];

  return (
    <>
      <SettingsGroup title="Anywhere in Windows">
        <SettingsRow
          label="Show or hide the launcher"
          description="Works while any app has the focus."
          control={<Keys shortcut={global.toggle.toLowerCase()} />}
        />
      </SettingsGroup>
      <SettingsGroup title="In the launcher">
        {rows.map(([label, shortcut]) => (
          <SettingsRow key={label} label={label} control={<Keys shortcut={shortcut} />} />
        ))}
      </SettingsGroup>
      <div className="flex items-center justify-between gap-4 px-1">
        <p className="text-fg-muted text-xs">
          Change a shortcut in <span className="font-mono">keybindings.toml</span>; it applies as
          soon as you save.
        </p>
        <Button
          size="sm"
          variant="secondary"
          leadingIcon="codicon:go-to-file"
          onClick={() => backend.openConfigFile('keybindings').catch(report)}
        >
          Edit keybindings.toml
        </Button>
      </div>
    </>
  );
}

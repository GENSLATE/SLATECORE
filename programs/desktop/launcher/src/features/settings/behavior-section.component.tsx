import { Badge, Button, Switch } from '@genslate/design-system';

import { useLauncher } from '../../app/launcher.context';
import { SettingsGroup, SettingsRow } from './settings-row.component';
import { useReport } from './use-report.hook';

function State({ on }: { on: boolean }) {
  return (
    <Badge tone={on ? 'accent' : 'neutral'} variant="subtle" size="md" dot>
      {on ? 'On' : 'Off'}
    </Badge>
  );
}

/**
 * When the launcher hides and whether it stays on top. Pin applies at once; the hide rules are
 * read from settings.toml and shown here with a shortcut to the file.
 */
export function BehaviorSection() {
  const { backend, settings, pinned } = useLauncher();
  const report = useReport();
  const { hideOnBlur, hideOnLaunch } = settings.config.behavior;

  return (
    <>
      <SettingsGroup title="Window">
        <SettingsRow
          labelId="settings-pinned"
          label="Pin on top"
          description="Keep the launcher open when you click elsewhere."
          control={
            <Switch
              aria-labelledby="settings-pinned"
              checked={pinned}
              onCheckedChange={(checked) => backend.setPinned(checked).catch(report)}
            />
          }
        />
        <SettingsRow
          label="Hide when you click elsewhere"
          description="While the launcher is not pinned."
          control={<State on={hideOnBlur} />}
        />
        <SettingsRow
          label="Hide after launching an app"
          description="The app opens and the launcher steps out of the way."
          control={<State on={hideOnLaunch} />}
        />
      </SettingsGroup>
      <div className="flex items-center justify-between gap-4 px-1">
        <p className="text-fg-muted text-xs">
          The hide rules live in <span className="font-mono">settings.toml</span>; the launcher
          picks up changes as you save.
        </p>
        <Button
          size="sm"
          variant="secondary"
          leadingIcon="codicon:go-to-file"
          onClick={() => backend.openConfigFile('settings').catch(report)}
        >
          Edit settings.toml
        </Button>
      </div>
    </>
  );
}

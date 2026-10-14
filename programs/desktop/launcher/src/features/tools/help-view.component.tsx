import { Kbd } from '@genslate/design-system';

import type { ActionSpec, Keybindings } from '../../ipc/launcher.types';
import { PanelSheet } from './panel-sheet.component';

export interface HelpViewProps {
  readonly actions: readonly ActionSpec[];
  readonly keybindings: Keybindings;
  readonly onBack: () => void;
}

/** Shortcuts and slash commands, straight from keybindings.toml and the command registry. */
export function HelpView({ actions, keybindings, onBack }: HelpViewProps) {
  const keys = keybindings.launcher;
  // Each row: a label and one or more shortcuts (an empty one is switched off).
  const shortcuts: readonly (readonly [string, readonly string[]])[] = [
    ['Show / hide anywhere', [keybindings.global.toggle.toLowerCase()]],
    ['Search', [keys.focusSearch]],
    ['Move', ['up', 'down']],
    ['Open', ['enter']],
    ['Favorite', [keys.toggleFavorite]],
    ['Settings', [keys.toggleTools]],
    ['Pin on top', [keys.togglePin]],
    ['Switch tab', [keys.tabGenslate, keys.tabPortapps, keys.tabPortableapps]],
    ['Back / hide', ['escape']],
  ];
  return (
    <PanelSheet title="Help" onBack={onBack}>
      <h3 className="mb-1.5 font-semibold text-2xs text-fg-muted uppercase tracking-wider">
        Shortcuts
      </h3>
      <dl className="mb-4 flex flex-col">
        {shortcuts
          .map(([label, list]) => [label, list.filter((shortcut) => shortcut !== '')] as const)
          .filter(([, list]) => list.length > 0)
          .map(([label, list]) => (
            <div key={label} className="flex h-7 items-center justify-between gap-2 text-sm">
              <dt className="text-fg-secondary">{label}</dt>
              <dd className="flex items-center gap-1">
                {list.map((shortcut) => (
                  <Kbd key={shortcut} shortcut={shortcut} size="sm" />
                ))}
              </dd>
            </div>
          ))}
      </dl>
      <h3 className="mb-1.5 font-semibold text-2xs text-fg-muted uppercase tracking-wider">
        Commands
      </h3>
      <dl className="flex flex-col gap-1">
        {actions.map((action) => (
          <div key={action.id} className="flex flex-col py-0.5">
            <dt className="font-mono text-fg-strong text-sm">
              /{action.id}
              {action.params[0] === undefined ? null : (
                <span className="text-fg-muted">
                  {' '}
                  ‹{action.params[0].description.toLowerCase()}›
                </span>
              )}
            </dt>
            <dd className="text-fg-muted text-xs">{action.description}</dd>
          </div>
        ))}
      </dl>
      <p className="mt-4 text-fg-muted text-xs">
        Settings live on this drive in{' '}
        <span className="font-mono">other/launcher/configs/settings.toml</span> and{' '}
        <span className="font-mono">keybindings.toml</span>. Edit them and the launcher updates as
        you save, or use <span className="font-mono">/settings</span>.
      </p>
    </PanelSheet>
  );
}

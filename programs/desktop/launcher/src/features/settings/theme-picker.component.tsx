import { cn } from '@genslate/design-system';

import type { ThemeSetting } from '../../ipc/launcher.types';

type PreviewTheme = Exclude<ThemeSetting, 'system'>;

const THEMES: readonly { readonly value: ThemeSetting; readonly label: string }[] = [
  { value: 'polar-night', label: 'Polar Night' },
  { value: 'snow-storm', label: 'Snow Storm' },
  { value: 'system', label: 'System' },
];

export interface ThemePickerProps {
  readonly value: ThemeSetting;
  readonly onValueChange: (theme: ThemeSetting) => void;
  /** Id of the visible label that names the group. */
  readonly labelledBy: string;
}

/**
 * Three cards, each a small launcher drawn with that theme's own tokens (`data-theme` scopes
 * them), so the choice is seen rather than read. System shows both halves.
 */
export function ThemePicker({ value, onValueChange, labelledBy }: ThemePickerProps) {
  return (
    // biome-ignore lint/a11y/useSemanticElements: a fieldset would add a border and a legend; this is a labelled group of toggle buttons.
    <div role="group" aria-labelledby={labelledBy} className="grid grid-cols-3 gap-2">
      {THEMES.map((theme) => {
        const selected = theme.value === value;
        return (
          <button
            key={theme.value}
            type="button"
            aria-pressed={selected}
            data-selected={selected || undefined}
            onClick={() => onValueChange(theme.value)}
            className={cn(
              'focus-ring flex cursor-interactive flex-col gap-2 rounded-card border p-1.5 pb-2 text-left',
              'transition-colors duration-fast ease-standard',
              selected
                ? 'border-accent bg-accent-subtle'
                : 'border-border-subtle hover:border-border hover:bg-fill-hover',
            )}
          >
            {theme.value === 'system' ? (
              <span
                data-slot="theme-preview"
                aria-hidden
                className="grid h-14 grid-cols-2 overflow-hidden rounded-md"
              >
                <MiniLauncher theme="polar-night" className="rounded-r-none border-r-0" />
                <MiniLauncher theme="snow-storm" className="rounded-l-none border-l-0" />
              </span>
            ) : (
              <MiniLauncher theme={theme.value} slot className="h-14" />
            )}
            <span className="flex items-center gap-2 px-1">
              <span
                aria-hidden
                className={cn(
                  'grid size-3.5 shrink-0 place-items-center rounded-full border transition-colors duration-fast ease-standard',
                  selected ? 'border-accent bg-accent' : 'border-border-strong bg-field',
                )}
              >
                <span
                  className={cn(
                    'size-1.5 rounded-full bg-on-accent transition-transform duration-base ease-emphasized',
                    selected ? 'scale-100' : 'scale-0',
                  )}
                />
              </span>
              <span
                className={cn(
                  'truncate font-medium text-base',
                  selected ? 'text-fg-strong' : 'text-fg-secondary',
                )}
              >
                {theme.label}
              </span>
            </span>
          </button>
        );
      })}
    </div>
  );
}

/** A launcher in miniature: rail, two app rows and the search bar, in `theme`'s colors. */
function MiniLauncher({
  theme,
  slot = false,
  className,
}: {
  theme: PreviewTheme;
  /** Marks this as the card's preview (System marks its wrapper instead). */
  slot?: boolean;
  className?: string;
}) {
  return (
    <span
      data-slot={slot ? 'theme-preview' : undefined}
      data-theme={theme}
      aria-hidden
      className={cn('flex overflow-hidden rounded-md border border-border bg-surface', className)}
    >
      <span className="w-1/4 shrink-0 border-border-subtle border-r bg-surface-sidebar" />
      <span className="flex min-w-0 flex-1 flex-col justify-between p-1.5">
        {[0, 1].map((row) => (
          <span key={row} className="flex items-center gap-1">
            <span
              className={cn(
                'size-2.5 shrink-0 rounded-xs',
                row === 0 ? 'bg-accent' : 'bg-fill-pressed',
              )}
            />
            <span className="h-1 w-3/5 rounded-full bg-fg-disabled" />
          </span>
        ))}
        <span className="h-2 rounded-xs border border-border-subtle bg-field" />
      </span>
    </span>
  );
}

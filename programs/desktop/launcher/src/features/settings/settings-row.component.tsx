import { cn } from '@genslate/design-system';
import type { ReactNode } from 'react';

export interface SettingsGroupProps {
  /** Small uppercase caption above the group. */
  readonly title?: string | undefined;
  readonly children: ReactNode;
  readonly className?: string | undefined;
}

/** A flat card of settings rows, separated by 1px dividers. */
export function SettingsGroup({ title, children, className }: SettingsGroupProps) {
  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      {title === undefined ? null : (
        <h3 className="px-1 font-semibold text-2xs text-fg-muted uppercase tracking-wider">
          {title}
        </h3>
      )}
      <div className="flex flex-col divide-y divide-border-subtle rounded-card border border-border-subtle bg-surface-raised">
        {children}
      </div>
    </div>
  );
}

export interface SettingsRowProps {
  readonly label: ReactNode;
  readonly description?: ReactNode | undefined;
  /** The control on the right. */
  readonly control?: ReactNode | undefined;
  /** Id of the label, for controls named by it (`aria-labelledby`). */
  readonly labelId?: string | undefined;
  /** Puts the control under the text, full width (for wide controls such as the theme cards). */
  readonly stacked?: boolean | undefined;
  readonly className?: string | undefined;
}

/** One setting: label and description on the left, its control on the right (or below). */
export function SettingsRow({
  label,
  description,
  control,
  labelId,
  stacked = false,
  className,
}: SettingsRowProps) {
  return (
    <div
      className={cn(
        'flex min-h-12 gap-4 px-3.5 py-2.5',
        stacked ? 'flex-col items-stretch gap-3 pb-3.5' : 'items-center',
        className,
      )}
    >
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span id={labelId} className="font-medium text-base text-fg-strong">
          {label}
        </span>
        {description === undefined ? null : (
          <span className="text-fg-muted text-xs leading-4.5">{description}</span>
        )}
      </div>
      {control === undefined ? null : (
        <div className={stacked ? 'min-w-0' : 'shrink-0'}>{control}</div>
      )}
    </div>
  );
}

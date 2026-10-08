import { Badge, IconButton } from '@genslate/design-system';

import markUrl from '../../assets/launcher-mark.svg';
import type { RunMode } from '../../ipc/launcher.types';

const MODE_BADGE: Readonly<Record<RunMode, string | null>> = {
  suite: null,
  dev: 'DEV',
  web: 'PREVIEW',
};

export interface LauncherTitleBarProps {
  /** Where the launcher runs from: a development build or the browser preview say so. */
  readonly mode: RunMode;
  readonly pinned: boolean;
  readonly pinShortcut: string;
  readonly onTogglePin: () => void;
  readonly onHide: () => void;
}

/**
 * The launcher's own titlebar: the mark and the SLATECORE LAUNCHER wordmark on the left; pin
 * and hide on the right. The window is fixed to the corner of the screen, so nothing drags it;
 * hiding sends it back to the notification area.
 */
export function LauncherTitleBar({
  mode,
  pinned,
  pinShortcut,
  onTogglePin,
  onHide,
}: LauncherTitleBarProps) {
  const badge = MODE_BADGE[mode];
  return (
    <header
      data-slot="launcher-titlebar"
      data-context-zone="titlebar"
      className="hairline-b flex h-full items-center gap-0.5 pr-1.5 pl-3.5"
    >
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <img src={markUrl} alt="" className="size-4.5 shrink-0" draggable={false} />
        <span className="truncate text-xs tracking-[0.14em]">
          <span className="font-semibold text-fg-strong">SLATECORE</span>{' '}
          <span className="font-medium text-fg-muted">LAUNCHER</span>
        </span>
        {badge === null ? null : (
          <Badge size="sm" className="font-semibold tracking-wider">
            {badge}
          </Badge>
        )}
      </div>
      <IconButton
        size="sm"
        label={pinned ? 'Unpin' : 'Pin on top'}
        icon={pinned ? 'codicon:pinned' : 'codicon:pin'}
        toggled={pinned}
        tooltipShortcut={pinShortcut}
        onClick={onTogglePin}
      />
      <IconButton
        size="sm"
        label="Hide to tray"
        icon="codicon:chrome-minimize"
        tooltipShortcut="escape"
        onClick={onHide}
      />
    </header>
  );
}

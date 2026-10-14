import { cn } from '@genslate/design-system';
import type { ReactNode } from 'react';

import { useLauncher } from '../../app/launcher.context';
import type { SizePreset } from '../../ipc/launcher.types';

export interface LauncherFrameProps {
  /** A tool is open: the frame widens to the left. */
  readonly expanded: boolean;
  /** Window height preset (S, M, L). */
  readonly size: SizePreset;
  readonly titleBar: ReactNode;
  /** The documents rail, on the left. */
  readonly rail: ReactNode;
  /** The tool-button row at the bottom of the rail, level with the command bar. */
  readonly railFooter: ReactNode;
  /** The app list, its sub-views and the open tool. */
  readonly main: ReactNode;
  /** Search + slash bar, under the main panel. */
  readonly commandBar: ReactNode;
  readonly statusBar: ReactNode;
}

/**
 * The launcher frame: one flat surface with a 1px border, pinned to the bottom-right of the
 * fixed-size window. The rail is a color step darker and meets the main panel at a hairline.
 * Opening a tool widens the frame from NORMAL to EXPANDED: the frame is laid out at the expanded
 * width and revealed with a clip, the left-hugging parts slide (`.launcher-frame`,
 * `.launcher-slide`, styles/main.css), so no layout property is animated.
 */
export function LauncherFrame({
  expanded,
  size,
  titleBar,
  rail,
  railFooter,
  main,
  commandBar,
  statusBar,
}: LauncherFrameProps) {
  const { stage } = useLauncher();
  return (
    <div
      data-slot="launcher-window"
      data-expanded={expanded || undefined}
      data-size={size}
      className="group/window chrome flex h-full items-end justify-end p-(--launcher-inset)"
    >
      <div
        data-slot="launcher-stage"
        data-state={stage}
        className="launcher-stage flex h-full items-end"
      >
        <div data-slot="launcher-frame" className="launcher-frame relative">
          <div
            data-slot="launcher-frame-body"
            className={cn(
              'launcher-frame-body relative grid size-full overflow-hidden bg-surface text-fg',
              'grid-cols-[var(--spacing-launcher-rail)_minmax(0,1fr)]',
              'grid-rows-[var(--spacing-launcher-titlebar)_minmax(0,1fr)_var(--spacing-launcher-band)_var(--spacing-launcher-status)]',
            )}
          >
            <div className="col-span-2 min-w-0">{titleBar}</div>
            <div className="launcher-slide hairline-r z-raised row-span-2 flex min-h-0 min-w-0 flex-col bg-surface-sidebar">
              <div className="min-h-0 flex-1">{rail}</div>
              <div className="hairline-t h-launcher-band shrink-0">{railFooter}</div>
            </div>
            <div className="relative min-h-0 min-w-0 overflow-hidden">{main}</div>
            <div className="hairline-t relative min-w-0">{commandBar}</div>
            <div className="col-span-2 min-w-0">{statusBar}</div>
          </div>
        </div>
      </div>
    </div>
  );
}

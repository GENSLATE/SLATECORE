import { Icon, IconButton, ProgressBar, Tooltip } from '@genslate/design-system';
import { type ReactNode, useEffect, useState } from 'react';

import { useLauncher } from '../../app/launcher.context';
import type { StatusMode, Telemetry, VolumeInfo } from '../../ipc/launcher.types';
import { formatBytes, formatPercent, formatRate, formatTemp } from './format.util';

export interface LauncherStatusBarProps {
  readonly mode: StatusMode;
  readonly onModeChange: (mode: StatusMode) => void;
  readonly onOpenSettingsFile: () => void;
}

/**
 * The frame's bottom edge: the drive the launcher runs from (letter, name, free space) on the
 * left; CPU/GPU temperatures, or usage and network, on the right. Readings the machine can't
 * provide are hidden; sampling only runs while the launcher is visible.
 */
export function LauncherStatusBar({
  mode,
  onModeChange,
  onOpenSettingsFile,
}: LauncherStatusBarProps) {
  const { backend, stage, showCount, settings, context } = useLauncher();
  const [volume, setVolume] = useState<VolumeInfo | null>(null);
  const [telemetry, setTelemetry] = useState<Telemetry | null>(null);

  useEffect(() => {
    if (showCount === 0) return;
    backend.volume().then(setVolume, () => setVolume(null));
  }, [backend, showCount]);

  useEffect(() => {
    const open = stage === 'open';
    const stop = backend.on('telemetry', setTelemetry);
    backend.setTelemetryActive(open).catch(() => undefined);
    return () => {
      stop.then(
        (unsubscribe) => unsubscribe(),
        () => undefined,
      );
      backend.setTelemetryActive(false).catch(() => undefined);
    };
  }, [backend, stage]);

  const hasTemps =
    telemetry !== null && (telemetry.cpuTempC !== null || telemetry.gpuTempC !== null);
  const effective: StatusMode = mode === 'temps' && !hasTemps ? 'usage' : mode;

  return (
    <footer
      data-slot="launcher-status-bar"
      data-context-zone="statusbar"
      data-has-temps={hasTemps || undefined}
      className="hairline-t flex h-full items-center gap-3 bg-surface-sidebar pr-1.5 pl-3 text-fg-muted text-xs tabular-nums"
    >
      {volume === null ? null : <DriveMeter volume={volume} />}
      <div className="ml-auto flex shrink-0 items-center gap-2.5">
        {settings.issue === null ? null : (
          <Tooltip content={`Settings file ignored: ${settings.issue}`}>
            <button
              type="button"
              onClick={onOpenSettingsFile}
              className="focus-ring flex cursor-default items-center gap-1 rounded-sm px-1 text-warning-fg hover:bg-fill-hover"
            >
              <Icon name="codicon:warning" size={12} />
              Settings
            </button>
          </Tooltip>
        )}
        {context.mode === 'suite' ? null : (
          <span className="rounded-sm bg-fill-hover px-1 font-semibold text-2xs tracking-wider">
            {context.mode === 'dev' ? 'DEV' : 'PREVIEW'}
          </span>
        )}
        {telemetry === null ? null : <Readings telemetry={telemetry} mode={effective} />}
        {hasTemps ? (
          <IconButton
            size="xs"
            label={effective === 'temps' ? 'Show usage' : 'Show temperatures'}
            icon="codicon:arrow-swap"
            onClick={() => onModeChange(effective === 'temps' ? 'usage' : 'temps')}
          />
        ) : null}
      </div>
    </footer>
  );
}

function DriveMeter({ volume }: { volume: VolumeInfo }) {
  const free = volume.totalBytes > 0 ? (volume.availableBytes / volume.totalBytes) * 100 : 0;
  const tone = free < 5 ? 'danger' : free < 15 ? 'warning' : 'accent';
  const name = volume.name ?? 'Drive';
  const title = `${name} (${volume.label}): ${formatBytes(volume.availableBytes)} free of ${formatBytes(volume.totalBytes)}${volume.removable ? ', removable' : ''}`;
  return (
    <Tooltip content={title} side="top">
      <span
        data-slot="drive-meter"
        data-context-copy={title}
        className="motion-fade-up flex min-w-0 cursor-default items-center gap-1.5"
      >
        <Icon name={volume.removable ? 'codicon:archive' : 'codicon:database'} size={12} />
        <span className="font-semibold text-fg-secondary">{volume.label}</span>
        <span className="truncate text-fg-secondary">{name}</span>
        <ProgressBar
          value={100 - free}
          size="md"
          tone={tone}
          aria-label="Space used"
          className="w-10 shrink-0"
        />
        <span className="shrink-0">{formatBytes(volume.availableBytes)} free</span>
      </span>
    </Tooltip>
  );
}

function Readings({ telemetry, mode }: { telemetry: Telemetry; mode: StatusMode }) {
  const items: ReactNode[] = [];
  if (mode === 'temps') {
    if (telemetry.cpuTempC !== null)
      items.push(<Reading key="cpu" label="CPU" value={formatTemp(telemetry.cpuTempC)} />);
    if (telemetry.gpuTempC !== null)
      items.push(<Reading key="gpu" label="GPU" value={formatTemp(telemetry.gpuTempC)} />);
  } else {
    if (telemetry.cpuUsagePct !== null)
      items.push(<Reading key="cpu" label="CPU" value={formatPercent(telemetry.cpuUsagePct)} />);
    if (telemetry.gpuUsagePct !== null)
      items.push(<Reading key="gpu" label="GPU" value={formatPercent(telemetry.gpuUsagePct)} />);
    if (telemetry.netDownBps !== null)
      items.push(<Reading key="down" icon="down" value={formatRate(telemetry.netDownBps)} />);
  }
  return (
    <span key={mode} className="motion-fade-up flex items-center gap-2.5" aria-live="off">
      {items}
    </span>
  );
}

function Reading({ label, icon, value }: { label?: string; icon?: 'down'; value: string }) {
  const name = label ?? 'Download';
  return (
    <span data-context-copy={`${name} ${value}`} className="flex items-center gap-1">
      {icon === undefined ? null : <Icon name="codicon:arrow-down" size={12} />}
      {label === undefined ? null : <span>{label}</span>}
      <span className="font-medium text-fg-secondary">{value}</span>
    </span>
  );
}

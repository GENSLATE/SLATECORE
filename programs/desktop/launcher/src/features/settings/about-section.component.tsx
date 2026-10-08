import { Badge, Icon, ProgressBar } from '@genslate/design-system';
import { useEffect, useState } from 'react';

import { useLauncher } from '../../app/launcher.context';
import markUrl from '../../assets/launcher-mark.svg';
import type { VolumeInfo } from '../../ipc/launcher.types';
import { formatBytes, formatPercent } from '../status/format.util';
import { describeInstall } from '../tray-menu/tray-menu.model';
import { SettingsGroup, SettingsRow } from './settings-row.component';

/** What the launcher keeps on the drive instead of the PC. */
const KEPT_ON_DRIVE: readonly string[] = [
  'Settings, keybindings, logs, caches and the app database',
  'The web view’s data and the temporary files of this session',
  'Decrypted vault files, wiped again when the vault locks',
];

/** What the launcher cannot control (spec §4, stated plainly). */
const MAY_STILL_RECORD: readonly { title: string; detail: string }[] = [
  {
    title: 'Windows itself',
    detail:
      'Recent files and jump lists, Prefetch entries for the programs you run, the notification area’s icon history, and the record that this drive was plugged in.',
  },
  {
    title: 'Other launchers and their apps',
    detail:
      'The PortableApps.com and portapps.io launchers, and the apps they start, may write settings, caches or crash reports to this PC on their own.',
  },
];

/**
 * About this drive: the product (SLATECORE LAUNCHER by GENSLATE, version), the drive it runs
 * from, what the launcher keeps on the drive, and what Windows and other launchers may still
 * record on the PC.
 */
export function AboutSection() {
  const { backend, context } = useLauncher();
  const [volume, setVolume] = useState<VolumeInfo | null>(null);

  useEffect(() => {
    backend.volume().then(setVolume, () => setVolume(null));
  }, [backend]);

  const free =
    volume !== null && volume.totalBytes > 0
      ? (volume.availableBytes / volume.totalBytes) * 100
      : 0;

  return (
    <>
      <div className="flex items-center gap-4 rounded-card border border-border-subtle bg-surface-raised px-4 py-4">
        <span className="grid size-14 shrink-0 place-items-center rounded-2xl border border-border-subtle bg-surface-sunken">
          <img src={markUrl} alt="" className="size-9" draggable={false} />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <h3 className="font-semibold text-fg-strong text-lg tracking-[0.08em]">
            SLATECORE LAUNCHER
          </h3>
          <p className="text-fg-secondary text-sm">by GENSLATE</p>
          <p className="text-fg-muted text-xs">Version {context.version}</p>
        </div>
        <Badge tone="accent" variant="subtle" size="md">
          v{context.version}
        </Badge>
      </div>

      <SettingsGroup title="This drive">
        <SettingsRow
          label={volume === null ? 'Drive' : `${volume.name ?? 'Drive'} (${volume.label})`}
          description={describeInstall(context)}
          control={
            volume === null ? null : (
              <Badge tone="neutral" variant="outline" size="md" icon="codicon:archive">
                {volume.removable ? 'Removable' : 'Fixed'}
              </Badge>
            )
          }
        />
        {volume === null ? null : (
          <div className="flex flex-col gap-2 px-3.5 py-3">
            <div className="flex items-center justify-between text-sm">
              <span className="text-fg-secondary">Free space</span>
              <span className="text-fg-strong tabular-nums">
                {formatBytes(volume.availableBytes)} of {formatBytes(volume.totalBytes)} ·{' '}
                {formatPercent(free)} free
              </span>
            </div>
            <ProgressBar
              value={100 - free}
              aria-label="Space used on this drive"
              tone={free < 5 ? 'danger' : free < 15 ? 'warning' : 'accent'}
            />
          </div>
        )}
        <SettingsRow
          label="Profile"
          control={<span className="text-fg-secondary text-sm">{context.profile}</span>}
        />
      </SettingsGroup>

      <SettingsGroup title="Kept on this drive">
        <ul className="flex flex-col gap-2 px-3.5 py-3">
          {KEPT_ON_DRIVE.map((item) => (
            <li key={item} className="flex items-start gap-2 text-fg-secondary text-sm">
              <Icon
                name="codicon:pass-filled"
                size={14}
                className="mt-0.5 shrink-0 text-success-fg"
              />
              {item}
            </li>
          ))}
        </ul>
        <p className="px-3.5 py-2.5 text-fg-muted text-xs leading-relaxed">
          Nothing is written to the registry or to your user folders, and drive letters are never
          saved, so the drive works the same on the next PC.
        </p>
      </SettingsGroup>

      <section aria-labelledby="about-may-record" className="flex flex-col gap-1.5">
        <h3
          id="about-may-record"
          className="px-1 font-semibold text-2xs text-fg-muted uppercase tracking-wider"
        >
          What this PC may still record
        </h3>
        <div className="flex flex-col divide-y divide-border-subtle rounded-card border border-warning-border bg-warning-subtle">
          {MAY_STILL_RECORD.map(({ title, detail }) => (
            <div key={title} className="flex items-start gap-3 px-3.5 py-3">
              <Icon name="codicon:info" size={14} className="mt-0.5 shrink-0 text-warning-fg" />
              <div className="flex flex-col gap-0.5">
                <span className="font-medium text-fg-strong text-sm">{title}</span>
                <span className="text-fg-secondary text-xs leading-relaxed">{detail}</span>
              </div>
            </div>
          ))}
        </div>
      </section>
    </>
  );
}

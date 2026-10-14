import {
  Badge,
  Button,
  FeatureTeaser,
  FeatureTeaserSample,
  Icon,
  Kbd,
  ProgressBar,
} from '@genslate/design-system';
import type { ReactNode } from 'react';

import { type ToolId, toolInfo } from './tools.model';

type TeaserTool = Exclude<ToolId, 'settings'>;

const COPY: Record<TeaserTool, string> = {
  manager:
    'Install, update and remove PortableApps.com and portapps.io apps without leaving the launcher, straight onto this drive.',
  storage:
    'See what fills this drive, and back up your folders and the vault to a second drive in one step.',
  diagnostics:
    'Check the health of this drive, find apps that fail to start and read what the launcher logged, in plain words.',
  ai: 'Ask in plain words: “open my notes and start the terminal in Documents”. Every launcher command is already one the assistant will be able to run for you.',
};

function SampleRow({ icon, label, detail }: { icon: ReactNode; label: string; detail: ReactNode }) {
  return (
    <div className="flex h-8 items-center gap-2.5 rounded-md px-2 text-sm">
      {icon}
      <span className="min-w-0 flex-1 truncate text-fg-strong">{label}</span>
      {detail}
    </div>
  );
}

const SAMPLES: Record<TeaserTool, ReactNode> = {
  manager: (
    <div className="flex flex-col">
      <SampleRow
        icon={<Icon name="codicon:package" size={14} />}
        label="Mozilla Firefox"
        detail={
          <Badge tone="accent" size="sm">
            Update 131.0
          </Badge>
        }
      />
      <SampleRow
        icon={<Icon name="codicon:package" size={14} />}
        label="GIMP"
        detail={
          <Badge tone="success" size="sm">
            Up to date
          </Badge>
        }
      />
      <SampleRow
        icon={<Icon name="codicon:archive" size={14} />}
        label="Signal"
        detail={
          <Badge tone="neutral" variant="outline" size="sm">
            Install
          </Badge>
        }
      />
    </div>
  ),
  storage: (
    <div className="flex flex-col gap-2.5 px-2">
      {(
        [
          ['Pictures', 48],
          ['Documents', 21],
          ['Vault', 9],
        ] as const
      ).map(([label, value]) => (
        <div key={label} className="flex items-center gap-3 text-sm">
          <span className="w-20 text-fg-secondary">{label}</span>
          <ProgressBar value={value} aria-label={label} className="flex-1" />
        </div>
      ))}
    </div>
  ),
  diagnostics: (
    <div className="flex flex-col">
      <SampleRow
        icon={<Icon name="codicon:pass-filled" size={14} className="text-success-fg" />}
        label="Drive health"
        detail={<span className="text-fg-muted text-xs">Good</span>}
      />
      <SampleRow
        icon={<Icon name="codicon:warning" size={14} className="text-warning-fg" />}
        label="Theater"
        detail={<span className="text-fg-muted text-xs">Program missing</span>}
      />
      <SampleRow
        icon={<Icon name="codicon:pulse" size={14} />}
        label="Last start"
        detail={<span className="text-fg-muted text-xs tabular-nums">1.2 s</span>}
      />
    </div>
  ),
  ai: (
    <div className="flex flex-col gap-2 px-2">
      <div className="self-end rounded-lg bg-accent-subtle px-3 py-1.5 text-fg-strong text-sm">
        Open my notes and the terminal in Documents
      </div>
      <div className="flex gap-1.5">
        <Badge tone="neutral" variant="outline" size="md" className="font-mono">
          /open Editor
        </Badge>
        <Badge tone="neutral" variant="outline" size="md" className="font-mono">
          /open Terminal
        </Badge>
      </div>
    </div>
  ),
};

export interface ToolTeaserProps {
  readonly tool: TeaserTool;
  readonly onClose: () => void;
}

/** A tool still to come: the design system's "Coming soon" teaser and a faded preview. */
export function ToolTeaser({ tool, onClose }: ToolTeaserProps) {
  const info = toolInfo(tool);
  return (
    <div className="flex h-full items-center justify-center overflow-y-auto px-10 py-6">
      <div className="flex w-full max-w-md flex-col gap-4 rounded-card border border-border-subtle bg-surface-raised pb-4">
        <FeatureTeaser icon={info.icon} title={info.label}>
          {COPY[tool]}
        </FeatureTeaser>
        <FeatureTeaserSample label="Preview">{SAMPLES[tool]}</FeatureTeaserSample>
        <div className="flex justify-end px-4">
          <Button variant="ghost" size="sm" leadingIcon="codicon:arrow-left" onClick={onClose}>
            Back to apps
            <Kbd shortcut="escape" variant="inline" size="sm" className="ml-1 text-fg-muted" />
          </Button>
        </div>
      </div>
    </div>
  );
}

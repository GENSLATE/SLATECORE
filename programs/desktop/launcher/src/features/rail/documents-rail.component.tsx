import type { CodiconRef } from '@genslate/design-system';
import { cn, Icon, Tooltip } from '@genslate/design-system';
import type { CSSProperties } from 'react';

import type { SharedFolder, VaultState } from '../../ipc/launcher.types';

const FOLDERS: readonly { folder: SharedFolder; label: string; icon: CodiconRef }[] = [
  { folder: 'desktop', label: 'Desktop', icon: 'codicon:vm' },
  { folder: 'documents', label: 'Documents', icon: 'codicon:file-text' },
  { folder: 'downloads', label: 'Downloads', icon: 'codicon:desktop-download' },
  { folder: 'music', label: 'Music', icon: 'codicon:music' },
  { folder: 'pictures', label: 'Pictures', icon: 'codicon:file-media' },
  { folder: 'videos', label: 'Videos', icon: 'codicon:device-camera-video' },
];

const VAULT_LABEL: Record<VaultState, string> = {
  uninitialized: 'Vault (not set up)',
  locked: 'Vault (locked)',
  unlocked: 'Vault (unlocked)',
};

export interface DocumentsRailProps {
  /** Profile name (`Shared`). */
  readonly profile: string;
  /** The install folder's name, shown as the drive name. */
  readonly suiteName: string | null;
  readonly vault: VaultState;
  readonly onOpenFolder: (folder: SharedFolder) => void;
  /** Opens Settings on its Vault section. */
  readonly onOpenVault: () => void;
}

const ROW = cn(
  'group/folder flex h-7 w-full cursor-default items-center gap-2 rounded-md px-2 text-fg-secondary text-sm',
  'focus-ring transition-colors duration-fast ease-standard hover:bg-fill-hover hover:text-fg-strong active:bg-fill-pressed',
);

/**
 * The left rail: who you are (profile card) and your portable folders
 * (`storage/users/shared/*`), then the encrypted vault, which opens in Settings.
 */
export function DocumentsRail({
  profile,
  suiteName,
  vault,
  onOpenFolder,
  onOpenVault,
}: DocumentsRailProps) {
  return (
    <nav
      aria-label="Your folders"
      data-slot="documents-rail"
      data-context-zone="rail"
      className="flex h-full flex-col px-2 pt-2"
    >
      <ProfileCard
        profile={profile}
        suiteName={suiteName}
        onOpenStorage={() => onOpenFolder('storage')}
      />
      <div
        id="rail-folders"
        className="mt-3 mb-1 px-2 font-semibold text-2xs text-fg-muted uppercase tracking-wider"
      >
        Folders
      </div>
      <ul aria-labelledby="rail-folders" className="flex flex-col gap-px">
        {FOLDERS.map(({ folder, label, icon }, index) => (
          <li
            key={folder}
            className="motion-row-in"
            style={{ '--stagger': index + 1 } as CSSProperties}
          >
            <button
              type="button"
              data-folder={folder}
              onClick={() => onOpenFolder(folder)}
              className={ROW}
            >
              <Icon
                name={icon}
                size={14}
                className="shrink-0 text-fg-muted transition-colors duration-fast ease-standard group-hover/folder:text-accent-fg"
              />
              <span className="truncate">{label}</span>
            </button>
          </li>
        ))}
        <li
          className="motion-row-in hairline-t mt-1.5 pt-1.5"
          style={{ '--stagger': FOLDERS.length + 1 } as CSSProperties}
        >
          <button
            type="button"
            data-vault={vault}
            aria-label={VAULT_LABEL[vault]}
            onClick={onOpenVault}
            className={ROW}
          >
            <Icon
              name={vault === 'unlocked' ? 'codicon:unlock' : 'codicon:lock'}
              size={14}
              className={cn(
                'shrink-0 transition-colors duration-fast ease-standard group-hover/folder:text-accent-fg',
                vault === 'unlocked' ? 'text-success-fg' : 'text-fg-muted',
              )}
            />
            <span className="truncate">Vault</span>
            <span
              aria-hidden
              className={cn(
                'ml-auto size-1.5 shrink-0 rounded-full',
                vault === 'unlocked'
                  ? 'bg-success'
                  : vault === 'locked'
                    ? 'bg-fg-disabled'
                    : 'bg-warning',
              )}
            />
          </button>
        </li>
      </ul>
    </nav>
  );
}

function ProfileCard({
  profile,
  suiteName,
  onOpenStorage,
}: {
  profile: string;
  suiteName: string | null;
  onOpenStorage: () => void;
}) {
  return (
    <Tooltip content="Open your storage folder" side="right">
      <button
        type="button"
        data-folder="storage"
        onClick={onOpenStorage}
        className={cn(
          'group/profile flex cursor-default items-center gap-2.5 rounded-lg px-2 py-2',
          'focus-ring transition-colors duration-fast ease-standard hover:bg-fill-hover active:bg-fill-pressed',
        )}
      >
        <span className="grid size-8 shrink-0 place-items-center rounded-full border border-accent-border bg-accent-subtle font-semibold text-accent-fg text-sm transition-transform duration-base ease-emphasized group-hover/profile:scale-105">
          {profile.charAt(0).toUpperCase()}
        </span>
        <span className="flex min-w-0 flex-col items-start">
          <span className="max-w-full truncate font-semibold text-fg-strong text-sm">
            {profile}
          </span>
          {suiteName === null ? null : (
            <span className="max-w-full truncate text-2xs text-fg-muted uppercase tracking-wider">
              {suiteName}
            </span>
          )}
        </span>
      </button>
    </Tooltip>
  );
}

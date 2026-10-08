import {
  Badge,
  Banner,
  Button,
  cn,
  EmptyState,
  Icon,
  PasswordField,
  useToast,
} from '@genslate/design-system';
import { type CSSProperties, type FormEvent, type ReactNode, useEffect, useState } from 'react';

import { useLauncher } from '../../app/launcher.context';
import { isVaultError } from '../../ipc/launcher.parse';
import { PREVIEW_VAULT_PASSWORD } from '../../ipc/launcher.preview';
import type { EntryDto, VaultStatusDto } from '../../ipc/launcher.types';
import { formatBytes } from '../status/format.util';
import { SettingsGroup } from './settings-row.component';
import { errorMessage, useReport } from './use-report.hook';

/** A vault error in words; it never repeats the password. */
export function vaultMessage(error: unknown): string {
  if (!isVaultError(error)) return errorMessage(error);
  switch (error.code) {
    case 'WRONG_PASSWORD':
      return 'Wrong password. Try again.';
    case 'THROTTLED':
      return `Too many wrong passwords. Try again in ${Math.ceil((error.retryAfterMs ?? 30_000) / 1000)} s.`;
    case 'PASSWORD_REJECTED':
      return 'Use 8 to 1024 characters.';
    default:
      return error.message;
  }
}

const NO_RECOVERY =
  'There is no password recovery. If you forget the vault password, nobody can open the files in it, not even GENSLATE.';

/**
 * The encrypted vault (`storage/vault`): set a password, unlock, lock, change the password and
 * open files. Passwords live only in the field while typed and are cleared the moment they are
 * sent; they never reach a store, a URL, a log or an error message.
 */
export function VaultSection() {
  const { vault } = useLauncher();
  return (
    <>
      <VaultStatusCard status={vault} />
      {vault.state === 'uninitialized' ? <CreateVault /> : null}
      {vault.state === 'locked' ? <UnlockVault status={vault} /> : null}
      {vault.state === 'unlocked' ? <UnlockedVault status={vault} /> : null}
      <Banner tone="warning" title="Keep the password safe">
        {NO_RECOVERY}
      </Banner>
    </>
  );
}

function VaultStatusCard({ status }: { status: VaultStatusDto }) {
  const unlocked = status.state === 'unlocked';
  const detail =
    status.state === 'uninitialized'
      ? 'Set a password to start encrypting the files you keep in storage/vault.'
      : unlocked
        ? `${status.entryCount ?? 0} files${status.sessionFiles > 0 ? ` · ${status.sessionFiles} open` : ''}. Opened files are decrypted into a session folder on this drive and wiped when the vault locks.`
        : `Unlock to open and add files.${status.failedAttempts > 0 ? ` ${status.failedAttempts} failed ${status.failedAttempts === 1 ? 'attempt' : 'attempts'}.` : ''}`;
  return (
    <div className="flex items-center gap-3.5 rounded-card border border-border-subtle bg-surface-raised px-4 py-3.5">
      <span
        className={cn(
          'grid size-10 shrink-0 place-items-center rounded-full border',
          unlocked
            ? 'border-success-border bg-success-subtle text-success-fg'
            : 'border-border bg-fill-hover text-fg-secondary',
        )}
      >
        <Icon name={unlocked ? 'codicon:unlock' : 'codicon:lock'} size={16} />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex items-center gap-2">
          <span className="font-semibold text-fg-strong text-md">Vault</span>
          <Badge
            tone={unlocked ? 'success' : status.state === 'locked' ? 'neutral' : 'warning'}
            size="sm"
            dot
          >
            {status.state === 'uninitialized' ? 'Not set up' : unlocked ? 'Unlocked' : 'Locked'}
          </Badge>
        </div>
        <p className="text-fg-muted text-xs leading-4.5">{detail}</p>
      </div>
    </div>
  );
}

function FormFooter({ children, hint }: { children: ReactNode; hint?: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 pt-1">
      <span className="min-w-0 text-fg-muted text-xs">{hint}</span>
      <div className="flex shrink-0 gap-2">{children}</div>
    </div>
  );
}

function CreateVault() {
  const { backend } = useLauncher();
  const [password, setPassword] = useState('');
  const [repeat, setRepeat] = useState('');
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if ([...password.normalize('NFKC')].length < 8) {
      setError('Use at least 8 characters.');
      return;
    }
    if (password !== repeat) {
      setError('The passwords do not match.');
      setRepeat('');
      return;
    }
    const secret = password;
    setPassword('');
    setRepeat('');
    setError(undefined);
    setBusy(true);
    backend
      .vaultCreate(secret)
      .catch((reason: unknown) => setError(vaultMessage(reason)))
      .finally(() => setBusy(false));
  };

  return (
    <SettingsGroup title="Set a vault password">
      <form className="flex flex-col gap-3 p-3.5" onSubmit={submit}>
        <PasswordField
          label="New vault password"
          description="8 to 1024 characters. A short sentence is easy to remember."
          value={password}
          onChange={(value) => {
            setPassword(value);
            setError(undefined);
          }}
        />
        <PasswordField
          label="Repeat the password"
          value={repeat}
          error={error}
          onChange={(value) => {
            setRepeat(value);
            setError(undefined);
          }}
        />
        <FormFooter hint="The vault opens as soon as it is created.">
          <Button
            type="submit"
            variant="primary"
            size="sm"
            loading={busy}
            leadingIcon="codicon:lock"
          >
            Create vault
          </Button>
        </FormFooter>
      </form>
    </SettingsGroup>
  );
}

function UnlockVault({ status }: { status: VaultStatusDto }) {
  const { backend, context } = useLauncher();
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (password === '') return;
    const secret = password;
    setPassword('');
    setBusy(true);
    backend
      .vaultUnlock(secret)
      .then(
        () => setError(undefined),
        (reason: unknown) => setError(vaultMessage(reason)),
      )
      .finally(() => setBusy(false));
  };

  const throttled = status.retryAfterMs !== null && status.retryAfterMs > 0;
  return (
    <SettingsGroup title="Unlock">
      <form className="flex flex-col gap-3 p-3.5" onSubmit={submit}>
        <PasswordField
          label="Vault password"
          value={password}
          error={error}
          onChange={(value) => {
            setPassword(value);
            setError(undefined);
          }}
        />
        <FormFooter
          hint={
            context.mode === 'web' ? (
              <>
                Browser preview: the demo password is{' '}
                <span className="font-mono text-fg-secondary">{PREVIEW_VAULT_PASSWORD}</span>
              </>
            ) : throttled ? (
              'Wait a moment before the next try.'
            ) : (
              'Unlocking takes a second: the key is derived on purpose slowly.'
            )
          }
        >
          <Button
            type="submit"
            variant="primary"
            size="sm"
            loading={busy}
            disabled={password === ''}
            leadingIcon="codicon:unlock"
          >
            Unlock
          </Button>
        </FormFooter>
      </form>
    </SettingsGroup>
  );
}

function parentOf(path: string): string {
  const slash = path.lastIndexOf('/');
  return slash === -1 ? '' : path.slice(0, slash);
}

function nameOf(path: string): string {
  return path.slice(path.lastIndexOf('/') + 1);
}

function UnlockedVault({ status }: { status: VaultStatusDto }) {
  const { backend } = useLauncher();
  const toast = useToast();
  const report = useReport();
  const [dir, setDir] = useState('');
  const [entries, setEntries] = useState<readonly EntryDto[] | null>(null);
  const [checking, setChecking] = useState(false);

  const revision = `${status.entryCount ?? 0}:${status.sessionFiles}`;
  // biome-ignore lint/correctness/useExhaustiveDependencies: `revision` reloads the list when the vault's content changes.
  useEffect(() => {
    let current = true;
    backend.vaultList(dir).then(
      (next) => {
        if (current) setEntries(next);
      },
      () => {
        if (current) setEntries([]);
      },
    );
    return () => {
      current = false;
    };
  }, [backend, dir, revision]);

  const open = (entry: EntryDto) => {
    if (entry.kind === 'dir') {
      setDir(entry.path);
      return;
    }
    backend.vaultOpen(entry.path).then(
      () =>
        toast.add({
          title: `Opened ${nameOf(entry.path)}`,
          description: 'Edits are encrypted again when you save.',
          type: 'success',
        }),
      (reason: unknown) => report(reason),
    );
  };

  const verify = () => {
    setChecking(true);
    backend
      .vaultVerify()
      .then(
        (result) =>
          toast.add(
            result.problems.length === 0
              ? { title: `All ${result.filesChecked} files are intact`, type: 'success' }
              : {
                  title: `${result.problems.length} files need attention`,
                  description: result.problems.map((problem) => problem.path).join(', '),
                  type: 'warning',
                },
          ),
        report,
      )
      .finally(() => setChecking(false));
  };

  const lock = () => {
    backend.vaultLock().then((result) => {
      if (result.unsynced.length > 0)
        toast.add({
          title: 'Some edits were not saved into the vault',
          description: result.unsynced.join(', '),
          type: 'warning',
        });
    }, report);
  };

  return (
    <>
      <div className="flex flex-wrap gap-2">
        <Button variant="primary" size="sm" leadingIcon="codicon:lock" onClick={lock}>
          Lock now
        </Button>
        <Button size="sm" leadingIcon="codicon:verified" loading={checking} onClick={verify}>
          Check files
        </Button>
      </div>

      <SettingsGroup title="Files">
        <div className="flex h-9 items-center gap-1 px-2 text-sm">
          <button
            type="button"
            onClick={() => setDir('')}
            className="focus-ring flex h-6 cursor-interactive items-center gap-1.5 rounded-sm px-1.5 text-fg-secondary hover:bg-fill-hover hover:text-fg-strong"
          >
            <Icon name="codicon:lock" size={12} />
            Vault
          </button>
          {dir === ''
            ? null
            : dir.split('/').map((part, index, parts) => (
                <span key={parts.slice(0, index + 1).join('/')} className="flex items-center gap-1">
                  <Icon name="codicon:chevron-right" size={12} className="text-fg-muted" />
                  <button
                    type="button"
                    onClick={() => setDir(parts.slice(0, index + 1).join('/'))}
                    className="focus-ring h-6 cursor-interactive rounded-sm px-1.5 text-fg-secondary hover:bg-fill-hover hover:text-fg-strong"
                  >
                    {part}
                  </button>
                </span>
              ))}
          {dir === '' ? null : (
            <Button
              size="xs"
              variant="ghost"
              leadingIcon="codicon:arrow-up"
              className="ml-auto"
              onClick={() => setDir(parentOf(dir))}
            >
              Up
            </Button>
          )}
        </div>
        {entries === null ? (
          <div className="h-24" />
        ) : entries.length === 0 ? (
          <EmptyState
            size="sm"
            icon="codicon:lock"
            title="Nothing here yet"
            description="Drop files into storage/vault on this drive: they are encrypted at the next unlock."
            className="py-6"
          />
        ) : (
          <ul aria-label="Vault files" className="flex flex-col p-1">
            {entries.map((entry, index) => (
              <li
                key={entry.path}
                className="motion-row-in"
                style={{ '--stagger': index } as CSSProperties}
              >
                <button
                  type="button"
                  onClick={() => open(entry)}
                  className="focus-ring group/entry flex h-9 w-full cursor-interactive items-center gap-2.5 rounded-md px-2.5 text-left text-sm transition-colors duration-fast ease-standard hover:bg-fill-hover"
                >
                  <Icon
                    name={entry.kind === 'dir' ? 'codicon:folder' : 'codicon:file-text'}
                    size={14}
                    className={entry.kind === 'dir' ? 'text-accent-fg' : 'text-fg-muted'}
                  />
                  <span className="min-w-0 flex-1 truncate text-fg-strong">
                    {nameOf(entry.path)}
                  </span>
                  <span className="text-fg-muted text-xs tabular-nums">
                    {entry.kind === 'dir' ? 'Folder' : formatBytes(entry.size)}
                  </span>
                  <span className="w-10 text-right text-accent-fg text-xs opacity-0 transition-opacity duration-fast ease-standard group-hover/entry:opacity-100">
                    Open
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </SettingsGroup>

      <ChangePassword />

      <p className="px-1 text-fg-muted text-xs leading-4.5">
        The vault locks itself after 10 minutes without use, when the launcher stays hidden, and
        when it quits.
        {status.foreignItems > 0
          ? ` ${status.foreignItems} plain files dropped into storage/vault were encrypted.`
          : ''}
      </p>
    </>
  );
}

function ChangePassword() {
  const { backend } = useLauncher();
  const toast = useToast();
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [repeat, setRepeat] = useState('');
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (next !== repeat) {
      setError('The new passwords do not match.');
      setRepeat('');
      return;
    }
    const secrets = { current, next };
    setCurrent('');
    setNext('');
    setRepeat('');
    setBusy(true);
    backend
      .vaultChangePassword(secrets.current, secrets.next)
      .then(
        () => {
          setError(undefined);
          toast.add({ title: 'Vault password changed', type: 'success' });
        },
        (reason: unknown) => setError(vaultMessage(reason)),
      )
      .finally(() => setBusy(false));
  };

  return (
    <SettingsGroup title="Change password">
      <form className="grid grid-cols-1 gap-3 p-3.5" onSubmit={submit}>
        <PasswordField label="Current password" value={current} onChange={setCurrent} />
        <div className="grid grid-cols-2 gap-3">
          <PasswordField label="New password" value={next} onChange={setNext} />
          <PasswordField
            label="Repeat new password"
            value={repeat}
            error={error}
            onChange={setRepeat}
          />
        </div>
        <FormFooter>
          <Button
            type="submit"
            size="sm"
            loading={busy}
            disabled={current === '' || next === '' || repeat === ''}
            leadingIcon="codicon:key"
          >
            Change password
          </Button>
        </FormFooter>
      </form>
    </SettingsGroup>
  );
}

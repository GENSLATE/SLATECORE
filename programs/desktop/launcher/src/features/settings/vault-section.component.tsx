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
import {
  type CSSProperties,
  type FormEvent,
  type ReactNode,
  type RefObject,
  useEffect,
  useRef,
  useState,
} from 'react';

import { useLauncher } from '../../app/launcher.context';
import { isVaultError } from '../../ipc/launcher.parse';
import { PREVIEW_VAULT_PASSWORD } from '../../ipc/launcher.preview';
import type { EntryDto, VaultStatusDto } from '../../ipc/launcher.types';
import { VAULT_PASSWORD_MAX_BYTES, vaultPasswordProblem } from '../../ipc/vault-password.util';
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
      return 'Use at least 8 characters and no more than 1024 bytes.';
    default:
      return error.message;
  }
}

/** What the password rule says about a new password, in words (`undefined`: it is fine). */
function newPasswordMessage(password: string): string | undefined {
  switch (vaultPasswordProblem(password)) {
    case 'too-short':
      return 'Use at least 8 characters.';
    case 'too-long':
      return `Too long: use at most ${VAULT_PASSWORD_MAX_BYTES} bytes.`;
    case null:
      return undefined;
  }
}

/** A form error and the field it belongs under. */
interface FieldError<Field extends string> {
  readonly field: Field;
  readonly message: string;
}

type PasswordRef = RefObject<HTMLInputElement | null>;

/** The secret in a password field, read straight from the input (never from React state). */
function readSecret(ref: PasswordRef): string {
  return ref.current?.value ?? '';
}

/** Empties a password field. */
function clearSecret(ref: PasswordRef): void {
  if (ref.current !== null) ref.current.value = '';
}

/** Reads a password field and empties it at once: the call that sends it is the only copy. */
function takeSecret(ref: PasswordRef): string {
  const secret = readSecret(ref);
  clearSecret(ref);
  return secret;
}

const NO_RECOVERY =
  'There is no password recovery. If you forget the vault password, nobody can open the files in it, not even GENSLATE.';

/**
 * The encrypted vault (`storage/vault`): set a password, unlock, lock, change the password and
 * open files. Password fields are uncontrolled: the secret lives only in the `<input>` while it
 * is typed, is read from it on submit and cleared straight away, and never reaches React state,
 * a store, a URL, a log or an error message.
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
  const passwordRef = useRef<HTMLInputElement>(null);
  const repeatRef = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<FieldError<'password' | 'repeat'> | undefined>();
  const [busy, setBusy] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const problem = newPasswordMessage(readSecret(passwordRef));
    if (problem !== undefined) {
      setError({ field: 'password', message: problem });
      return;
    }
    if (readSecret(passwordRef) !== readSecret(repeatRef)) {
      clearSecret(repeatRef);
      setError({ field: 'repeat', message: 'The passwords do not match.' });
      return;
    }
    clearSecret(repeatRef);
    setError(undefined);
    setBusy(true);
    backend
      .vaultCreate(takeSecret(passwordRef))
      .catch((reason: unknown) => setError({ field: 'password', message: vaultMessage(reason) }))
      .finally(() => setBusy(false));
  };

  const clearError = (field: 'password' | 'repeat') => {
    if (error?.field === field) setError(undefined);
  };

  return (
    <SettingsGroup title="Set a vault password">
      <form className="flex flex-col gap-3 p-3.5" onSubmit={submit}>
        <PasswordField
          ref={passwordRef}
          label="New vault password"
          description="At least 8 characters. A short sentence is easy to remember."
          error={error?.field === 'password' ? error.message : undefined}
          onInput={() => clearError('password')}
        />
        <PasswordField
          ref={repeatRef}
          label="Repeat the password"
          error={error?.field === 'repeat' ? error.message : undefined}
          onInput={() => clearError('repeat')}
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
  const passwordRef = useRef<HTMLInputElement>(null);
  // Whether the field has anything in it (enables Unlock); never the password itself.
  const [filled, setFilled] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const secret = takeSecret(passwordRef);
    setFilled(false);
    if (secret === '') return;
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
          ref={passwordRef}
          label="Vault password"
          error={error}
          onInput={(event) => {
            setFilled(event.currentTarget.value !== '');
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
            disabled={!filled}
            leadingIcon="codicon:unlock"
          >
            Unlock
          </Button>
        </FormFooter>
      </form>
    </SettingsGroup>
  );
}

/** The file list of the open folder: loading, read, or why it could not be read. */
type Listing =
  | { readonly kind: 'loading' }
  | { readonly kind: 'ready'; readonly entries: readonly EntryDto[] }
  | { readonly kind: 'failed'; readonly message: string };

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
  const [listing, setListing] = useState<Listing>({ kind: 'loading' });
  const [attempt, setAttempt] = useState(0);
  const [checking, setChecking] = useState(false);

  const revision = `${status.entryCount ?? 0}:${status.sessionFiles}:${attempt}`;
  // biome-ignore lint/correctness/useExhaustiveDependencies: `revision` reloads the list when the vault's content changes (or on Try again).
  useEffect(() => {
    let current = true;
    backend.vaultList(dir).then(
      (entries) => {
        if (current) setListing({ kind: 'ready', entries });
      },
      (reason: unknown) => {
        if (current) setListing({ kind: 'failed', message: vaultMessage(reason) });
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
        {listing.kind === 'loading' ? (
          <div className="h-24" />
        ) : listing.kind === 'failed' ? (
          <EmptyState
            size="sm"
            icon="codicon:error"
            title="Could not read the vault"
            description={listing.message}
            actions={
              <Button
                size="sm"
                variant="secondary"
                leadingIcon="codicon:refresh"
                onClick={() => setAttempt((count) => count + 1)}
              >
                Try again
              </Button>
            }
            className="py-6"
          />
        ) : listing.entries.length === 0 ? (
          <EmptyState
            size="sm"
            icon="codicon:lock"
            title="Nothing here yet"
            description="Drop files into storage/vault on this drive: they are encrypted at the next unlock."
            className="py-6"
          />
        ) : (
          <ul aria-label="Vault files" className="flex flex-col p-1">
            {listing.entries.map((entry, index) => (
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

type PasswordSlot = 'current' | 'next' | 'repeat';

/**
 * Current, new and repeated password. Only the current password is cleared the moment it is
 * sent: if it was wrong (or the vault is throttled) the error shows under it and the new
 * password stays typed, so only the current one has to be entered again.
 */
function ChangePassword() {
  const { backend } = useLauncher();
  const toast = useToast();
  const refs: Readonly<Record<PasswordSlot, PasswordRef>> = {
    current: useRef<HTMLInputElement>(null),
    next: useRef<HTMLInputElement>(null),
    repeat: useRef<HTMLInputElement>(null),
  };
  // Which fields have something in them (enables the button); never the passwords themselves.
  const [filled, setFilled] = useState<Readonly<Record<PasswordSlot, boolean>>>({
    current: false,
    next: false,
    repeat: false,
  });
  const [error, setError] = useState<FieldError<PasswordSlot> | undefined>();
  const [busy, setBusy] = useState(false);

  const empty = (...slots: readonly PasswordSlot[]) => {
    for (const slot of slots) clearSecret(refs[slot]);
    setFilled((current) => ({
      ...current,
      ...Object.fromEntries(slots.map((slot) => [slot, false])),
    }));
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const problem = newPasswordMessage(readSecret(refs.next));
    if (problem !== undefined) {
      setError({ field: 'next', message: problem });
      return;
    }
    if (readSecret(refs.next) !== readSecret(refs.repeat)) {
      empty('repeat');
      setError({ field: 'repeat', message: 'The new passwords do not match.' });
      return;
    }
    const current = takeSecret(refs.current);
    empty('current');
    setError(undefined);
    setBusy(true);
    backend
      .vaultChangePassword(current, readSecret(refs.next))
      .then(
        () => {
          empty('next', 'repeat');
          toast.add({ title: 'Vault password changed', type: 'success' });
        },
        (reason: unknown) => {
          const code = isVaultError(reason) ? reason.code : undefined;
          if (code === 'WRONG_PASSWORD' || code === 'THROTTLED') {
            setError({ field: 'current', message: vaultMessage(reason) });
            return;
          }
          empty('next', 'repeat');
          setError({ field: 'next', message: vaultMessage(reason) });
        },
      )
      .finally(() => setBusy(false));
  };

  const fieldProps = (slot: PasswordSlot) => ({
    ref: refs[slot],
    error: error?.field === slot ? error.message : undefined,
    onInput: (event: FormEvent<HTMLInputElement>) => {
      const value = event.currentTarget.value !== '';
      setFilled((current) => ({ ...current, [slot]: value }));
      if (error?.field === slot) setError(undefined);
    },
  });

  return (
    <SettingsGroup title="Change password">
      <form className="grid grid-cols-1 gap-3 p-3.5" onSubmit={submit}>
        <PasswordField label="Current password" {...fieldProps('current')} />
        <div className="grid grid-cols-2 items-start gap-3">
          <PasswordField label="New password" {...fieldProps('next')} />
          <PasswordField label="Repeat new password" {...fieldProps('repeat')} />
        </div>
        <FormFooter>
          <Button
            type="submit"
            size="sm"
            loading={busy}
            disabled={!(filled.current && filled.next && filled.repeat)}
            leadingIcon="codicon:key"
          >
            Change password
          </Button>
        </FormFooter>
      </form>
    </SettingsGroup>
  );
}

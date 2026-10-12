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
import type {
  EntryDto,
  LockBlockedDto,
  VaultErrorDto,
  VaultStatusDto,
} from '../../ipc/launcher.types';
import { isBlockedType } from '../../ipc/open-policy.util';
import {
  VAULT_PASSWORD_MAX_BYTES,
  VAULT_PASSWORD_MIN_BYTES,
  vaultPasswordProblem,
} from '../../ipc/vault-password.util';
import { formatBytes } from '../status/format.util';
import { SettingsGroup } from './settings-row.component';
import { errorMessage, useReport } from './use-report.hook';

/** Why a file the shell refuses to open (`BLOCKED_TYPE`) stays in the vault. */
const BLOCKED_TYPE_MESSAGE =
  "That file type can run code, so it can't be opened from the vault. Export it to a folder instead.";

/** A vault error in words; it never repeats the password. */
export function vaultMessage(error: unknown): string {
  if (!isVaultError(error)) return errorMessage(error);
  switch (error.code) {
    case 'WRONG_PASSWORD':
      return 'Wrong password. Try again.';
    case 'THROTTLED':
      return `Too many wrong passwords. Try again in ${Math.ceil((error.retryAfterMs ?? 30_000) / 1000)} s.`;
    case 'PASSWORD_REJECTED':
      return `Use ${VAULT_PASSWORD_MIN_BYTES} to ${VAULT_PASSWORD_MAX_BYTES} bytes.`;
    case 'BLOCKED_TYPE':
      return BLOCKED_TYPE_MESSAGE;
    default:
      return error.message;
  }
}

/**
 * What the password rule says about a new password, in words (`undefined`: it is fine). The
 * rule counts UTF-8 bytes after NFKC, so the copy says bytes: a plain letter is one, an accented
 * letter two, many symbols three or four.
 */
function newPasswordMessage(password: string): string | undefined {
  switch (vaultPasswordProblem(password)) {
    case 'too-short':
      return `Too short: use at least ${VAULT_PASSWORD_MIN_BYTES} bytes.`;
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

const WIPE_CAVEAT =
  'Wiping is best effort: on SSDs, USB flash drives and SD cards, and on some file systems, an overwrite may not reach the cells that held the file, so traces can survive. Full-drive encryption such as BitLocker To Go protects against that.';

/** `a`, `a and b`, `a, b, and c` (the serial comma keeps long clauses apart). */
function clauses(items: readonly string[]): string {
  if (items.length < 3) return items.join(' and ');
  return `${items.slice(0, -1).join(', ')}, and ${items.at(-1) ?? ''}`;
}

function minutes(count: number): string {
  return `${count} ${count === 1 ? 'minute' : 'minutes'}`;
}

/** When the vault locks itself, from `[vault]` in settings.toml (`0` turns a timer off). */
export function lockTimersMessage(idleMinutes: number, hideMinutes: number): string {
  const when = [
    ...(idleMinutes > 0 ? [`after ${minutes(idleMinutes)} without use`] : []),
    ...(hideMinutes > 0 ? [`${minutes(hideMinutes)} after the launcher is hidden`] : []),
    'when it quits',
  ];
  const never = [
    ...(idleMinutes === 0 ? ['on idle'] : []),
    ...(hideMinutes === 0 ? ['while hidden'] : []),
  ];
  return `Locks itself ${clauses(when)}${never.length === 0 ? '' : `, never ${never.join(' or ')}`}.`;
}

/** Plain items directly in storage/vault: waiting (locked) or left over by the unlock. */
function foreignMessage(status: VaultStatusDto): string | undefined {
  const count = status.foreignItems;
  if (count === 0) return undefined;
  const one = count === 1;
  const items = `${count} plain ${one ? 'item' : 'items'} in storage/vault`;
  if (status.state === 'locked')
    return `${items} ${one ? 'is' : 'are'} waiting to be encrypted: ${one ? 'it is' : 'they are'} imported when you unlock.`;
  if (status.state === 'unlocked')
    return `${items} ${one ? 'is' : 'are'} not encrypted yet: ${one ? 'it' : 'they'} could not be imported (in use or a link). The next unlock tries again.`;
  return undefined;
}

/** `Locked, but 2 decrypted files couldn't be deleted yet` (decrypted files a lock left). */
function leftoversTitle(count: number, locked: boolean): string {
  return `${locked ? 'Locked, but ' : ''}${count} decrypted ${count === 1 ? 'file' : 'files'} couldn't be deleted yet`;
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
  const { vault, vaultLeftovers, settings } = useLauncher();
  const timers = settings.config.vault;
  return (
    <>
      <VaultStatusCard status={vault} leftovers={vaultLeftovers} />
      {vault.state === 'uninitialized' ? <CreateVault /> : null}
      {vault.state === 'locked' ? <UnlockVault status={vault} /> : null}
      {vault.state === 'unlocked' ? <UnlockedVault status={vault} /> : null}
      <div className="flex flex-col gap-1.5 px-1 text-fg-muted text-xs leading-4.5">
        <p>
          {lockTimersMessage(timers.idleLockMinutes, timers.hideLockMinutes)} The timers are in
          settings.toml under [vault].
        </p>
        <p>{WIPE_CAVEAT}</p>
      </div>
      <Banner tone="warning" title="Keep the password safe">
        {NO_RECOVERY}
      </Banner>
    </>
  );
}

function VaultStatusCard({
  status,
  leftovers,
}: {
  status: VaultStatusDto;
  leftovers: readonly string[];
}) {
  const unlocked = status.state === 'unlocked';
  const foreign = foreignMessage(status);
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
        {foreign === undefined ? null : (
          <p className="text-fg-secondary text-xs leading-4.5">{foreign}</p>
        )}
        {leftovers.length === 0 ? null : (
          <div className="flex items-start gap-1.5 text-warning-fg text-xs leading-4.5">
            <Icon name="codicon:warning" size={12} className="mt-0.75 shrink-0" />
            <div className="flex min-w-0 flex-col">
              <p>
                {leftoversTitle(leftovers.length, status.state === 'locked')}: close the program
                using {leftovers.length === 1 ? 'it' : 'them'}. The launcher keeps retrying.
              </p>
              <p className="truncate text-fg-secondary">{leftovers.map(nameOf).join(', ')}</p>
            </div>
          </div>
        )}
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
          description={`At least ${VAULT_PASSWORD_MIN_BYTES} bytes (a plain letter is one). A short sentence is easy to remember.`}
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

/** `a`, `a and b`, `a, b and c`. */
function listOf(items: readonly string[]): string {
  if (items.length < 2) return items.join('');
  return `${items.slice(0, -1).join(', ')} and ${items.at(-1) ?? ''}`;
}

/** The files a lock could not save, from an `UNSYNCED_EDITS` error (empty for other errors). */
function unsavedOf(error: VaultErrorDto): readonly string[] {
  return error.code === 'UNSYNCED_EDITS' ? (error.paths ?? []) : [];
}

/** A lock from this view that failed on unsaved edits. */
interface LockFailure {
  readonly error: VaultErrorDto;
  /** The shell's notice at the time (a later one replaces this failure). */
  readonly seenWith: LockBlockedDto | undefined;
}

function UnlockedVault({ status }: { status: VaultStatusDto }) {
  const { backend, lockBlocked, vaultOpenNoteSeen, markVaultOpenNoteSeen, noteLockLeftovers } =
    useLauncher();
  const toast = useToast();
  const report = useReport();
  const [dir, setDir] = useState('');
  const [listing, setListing] = useState<Listing>({ kind: 'loading' });
  const [attempt, setAttempt] = useState(0);
  const [checking, setChecking] = useState(false);
  // The file waiting behind the traces note, and the last file the shell refused to open.
  const [pending, setPending] = useState<EntryDto | undefined>();
  const [refused, setRefused] = useState<string | undefined>();
  // Why the last lock from this view failed, and which shell notice was showing at the time.
  const [lockFailure, setLockFailure] = useState<LockFailure | undefined>();

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

  const decrypt = (entry: EntryDto) => {
    setPending(undefined);
    setRefused(undefined);
    backend.vaultOpen(entry.path).then(
      () =>
        toast.add({
          title: `Opened ${nameOf(entry.path)}`,
          description: 'Edits are encrypted again when you save.',
          type: 'success',
        }),
      (reason: unknown) => {
        if (isVaultError(reason) && reason.code === 'BLOCKED_TYPE') setRefused(entry.path);
        else report(reason);
      },
    );
  };

  const open = (entry: EntryDto) => {
    if (entry.kind === 'dir') {
      setDir(entry.path);
      return;
    }
    // A type the shell refuses is never decrypted, so it leaves no traces to warn about.
    if (vaultOpenNoteSeen || isBlockedType(entry.path)) decrypt(entry);
    else {
      setRefused(undefined);
      setPending(entry);
    }
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

  const lock = (force?: true) =>
    (force === true ? backend.vaultLock(true) : backend.vaultLock()).then(
      (result) => {
        setLockFailure(undefined);
        noteLockLeftovers(result.undeletable);
        if (result.undeletable.length > 0)
          toast.add({
            title: leftoversTitle(result.undeletable.length, true),
            description: `Close the program using ${listOf(result.undeletable.map(nameOf))}. The launcher keeps retrying.`,
            type: 'warning',
          });
        if (result.unsynced.length > 0)
          toast.add({
            title:
              force === true
                ? 'Vault locked, unsaved edits discarded'
                : 'Some edits were not saved into the vault',
            description: result.unsynced.join(', '),
            type: 'warning',
          });
      },
      (reason: unknown) => {
        // Edits the vault could not take back: say so where the lock buttons are, not in a toast.
        if (isVaultError(reason) && reason.code === 'UNSYNCED_EDITS')
          setLockFailure({ error: reason, seenWith: lockBlocked });
        else report(reason);
      },
    );

  return (
    <>
      <LockBlockedNotice
        error={
          // A newer notice from the shell supersedes what the last Lock again said.
          lockFailure !== undefined && lockFailure.seenWith === lockBlocked
            ? lockFailure.error
            : lockBlocked?.error
        }
        automatic={lockBlocked !== undefined}
        onLock={lock}
      />
      <div className="flex flex-wrap gap-2">
        <Button variant="primary" size="sm" leadingIcon="codicon:lock" onClick={() => lock()}>
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
        {pending === undefined ? null : (
          <OpenTracesNote
            name={nameOf(pending.path)}
            onOpen={() => {
              markVaultOpenNoteSeen();
              decrypt(pending);
            }}
            onLater={() => setPending(undefined)}
            onDismiss={() => {
              markVaultOpenNoteSeen();
              setPending(undefined);
            }}
          />
        )}
        {refused === undefined ? null : (
          <div className="px-2 pb-2">
            <Banner
              tone="warning"
              aria-label={`Can't open ${nameOf(refused)}`}
              title={`Can't open ${nameOf(refused)}`}
              onDismiss={() => setRefused(undefined)}
            >
              {BLOCKED_TYPE_MESSAGE}
            </Banner>
          </div>
        )}
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
    </>
  );
}

/**
 * Shown once per session before the first file is decrypted: Windows opens it through Explorer,
 * which records it like any other file.
 */
function OpenTracesNote({
  name,
  onOpen,
  onLater,
  onDismiss,
}: {
  name: string;
  onOpen: () => void;
  onLater: () => void;
  onDismiss: () => void;
}) {
  return (
    <div className="px-2 pb-2">
      <Banner
        tone="info"
        aria-label="Opening leaves traces on this PC"
        title="Opening leaves traces on this PC"
        onDismiss={onDismiss}
      >
        <p>
          Windows Explorer opens a plaintext copy, so the file name and its contents can show up in
          Recent Items and in the program's own recent lists and autosaves. Export leaves nothing
          behind.
        </p>
        <div className="mt-2 flex flex-wrap gap-2">
          <Button size="xs" variant="primary" leadingIcon="codicon:go-to-file" onClick={onOpen}>
            Open {name}
          </Button>
          <Button size="xs" variant="ghost" onClick={onLater}>
            Not now
          </Button>
        </div>
      </Banner>
    </div>
  );
}

/**
 * The vault could not lock (the shell's own lock on idle or hide, or the last Lock now): it stays
 * unlocked until the edits are saved, or the user locks anyway and loses them after confirming.
 */
function LockBlockedNotice({
  error,
  automatic,
  onLock,
}: {
  error: VaultErrorDto | undefined;
  automatic: boolean;
  onLock: (force?: true) => Promise<void>;
}) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  if (error === undefined) return null;

  const lead = automatic ? "Couldn't lock automatically" : "Couldn't lock";
  const unsaved = unsavedOf(error);
  const count = unsaved.length;
  const problem =
    error.code === 'UNSYNCED_EDITS'
      ? `${count} edited ${count === 1 ? "file couldn't" : "files couldn't"} be saved. Close the program that has ${count === 1 ? 'it' : 'them'} open, then lock again.`
      : `${vaultMessage(error)} Try to lock again.`;
  const run = (force?: true) => {
    setBusy(true);
    onLock(force).finally(() => {
      setBusy(false);
      setConfirming(false);
    });
  };

  return (
    <Banner tone={confirming ? 'danger' : 'warning'} aria-label={lead}>
      {confirming ? (
        <p>
          <span className="font-semibold text-fg-strong">Lock anyway?</span>{' '}
          {count === 0
            ? 'Edits that were not saved are lost.'
            : `Edits to ${listOf(unsaved)} are lost.`}{' '}
          The vault keeps the last saved {count === 1 ? 'version' : 'versions'}.
        </p>
      ) : (
        <p>
          <span className="font-semibold text-fg-strong">{lead}:</span> {problem}
        </p>
      )}
      <div className="mt-2 flex flex-wrap gap-2">
        {confirming ? (
          <>
            <Button
              size="xs"
              variant="danger"
              leadingIcon="codicon:lock"
              loading={busy}
              onClick={() => run(true)}
            >
              Discard edits and lock
            </Button>
            <Button size="xs" variant="ghost" onClick={() => setConfirming(false)}>
              Cancel
            </Button>
          </>
        ) : (
          <>
            <Button
              size="xs"
              variant="primary"
              leadingIcon="codicon:lock"
              loading={busy}
              onClick={() => run()}
            >
              Lock again
            </Button>
            <Button size="xs" variant="ghost" onClick={() => setConfirming(true)}>
              Lock and discard unsaved edits
            </Button>
          </>
        )}
      </div>
    </Banner>
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

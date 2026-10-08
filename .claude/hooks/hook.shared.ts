/**
 * Shared helpers for the Claude Code hooks in this folder.
 *
 * A hook reads one JSON payload on stdin and answers with its exit code: 0 is fine, 2 blocks the
 * tool call (stderr goes to Claude), anything else is ignored. Hooks fail open: a payload that
 * cannot be read, or an unexpected error, never stops the session.
 * https://code.claude.com/docs/en/hooks
 */
import { posix } from 'node:path';

/** The parts of a hook payload these hooks use. */
export interface HookInput {
  readonly eventName: string | undefined;
  readonly toolName: string | undefined;
  readonly toolInput: Readonly<Record<string, unknown>>;
}

const EMPTY: HookInput = { eventName: undefined, toolName: undefined, toolInput: {} };

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const text = (value: unknown): string | undefined =>
  typeof value === 'string' ? value : undefined;

/** Parses a hook payload; anything unreadable gives an empty input. */
export function parseInput(raw: string): HookInput {
  try {
    const value: unknown = JSON.parse(raw);
    if (!isRecord(value)) return EMPTY;
    const toolInput = value['tool_input'];
    return {
      eventName: text(value['hook_event_name']),
      toolName: text(value['tool_name']),
      toolInput: isRecord(toolInput) ? toolInput : {},
    };
  } catch {
    return EMPTY;
  }
}

/** Reads and parses stdin. */
export async function readInput(): Promise<HookInput> {
  try {
    return parseInput(await Bun.stdin.text());
  } catch {
    return EMPTY;
  }
}

/** The repository root Claude Code started in (falls back to the current directory). */
export function projectDir(): string {
  return process.env['CLAUDE_PROJECT_DIR'] || process.cwd();
}

const slashes = (path: string): string => path.replaceAll('\\', '/');

const isAbsolutePath = (path: string): boolean => path.startsWith('/') || /^[a-z]:\//i.test(path);

/**
 * `file` relative to `root` with `/` separators (Windows paths arrive with backslashes). A path
 * outside `root` comes back absolute, still with `/` separators.
 */
export function toRelative(file: string, root: string): string {
  const path = posix.normalize(slashes(file));
  const base = slashes(root).replace(/\/+$/, '');
  const windows = /^[a-z]:/i.test(base);
  const prefix = `${base}/`;
  const inside = windows
    ? path.toLowerCase().startsWith(prefix.toLowerCase())
    : path.startsWith(prefix);
  if (inside) return path.slice(prefix.length);
  return isAbsolutePath(path) ? path : path.replace(/^\.\//, '');
}

/** Whether a path from `toRelative` lies outside the project root (absolute, or climbing out). */
export function outsideProject(relPath: string): boolean {
  return isAbsolutePath(relPath) || relPath === '..' || relPath.startsWith('../');
}

/** The file an Edit, Write, MultiEdit or NotebookEdit call targets, relative to the project root. */
export function editedFile(input: HookInput, root: string = projectDir()): string | undefined {
  const file = input.toolInput['file_path'] ?? input.toolInput['notebook_path'];
  return typeof file === 'string' && file !== '' ? toRelative(file, root) : undefined;
}

/** Blocks the tool call: the message goes to Claude on stderr, exit code 2. */
export function block(message: string): never {
  console.error(message);
  process.exit(2);
}

/** Adds context for Claude after this hook event (stdout JSON, exit 0). */
export function addContext(eventName: string, additionalContext: string): void {
  console.log(
    JSON.stringify({ hookSpecificOutput: { hookEventName: eventName, additionalContext } }),
  );
}

/** Runs a hook body; an unexpected error is reported on stderr and never blocks anything. */
export async function run(main: () => Promise<void> | void): Promise<void> {
  try {
    await main();
  } catch (error) {
    console.error(`hook failed open: ${error instanceof Error ? error.message : String(error)}`);
  }
  process.exit(0);
}

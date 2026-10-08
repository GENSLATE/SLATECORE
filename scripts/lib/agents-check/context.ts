/** Shared helpers: the problem list, tolerant file access under a root, frontmatter. */
import { lstat, readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';

/** The repository root being checked and the problems found so far. */
export interface Context {
  readonly root: string;
  readonly problems: string[];
}

export const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

export const nonEmptyString = (value: unknown): value is string =>
  typeof value === 'string' && value.trim() !== '';

/** Records a problem; every problem starts with the path it is about. */
export function problem(ctx: Context, file: string, message: string): void {
  ctx.problems.push(`${file}: ${message}`);
}

/** Whether `rel` exists (a symlink counts, it is not followed). */
export async function exists(ctx: Context, rel: string): Promise<boolean> {
  try {
    await lstat(join(ctx.root, rel));
    return true;
  } catch {
    return false;
  }
}

export async function read(ctx: Context, rel: string): Promise<string | undefined> {
  try {
    return await readFile(join(ctx.root, rel), 'utf8');
  } catch {
    return undefined;
  }
}

/** Sorted entry names of a folder; a missing folder is empty. */
export async function list(ctx: Context, rel: string): Promise<string[]> {
  try {
    return (await readdir(join(ctx.root, rel))).sort();
  } catch {
    return [];
  }
}

/** A JSON object file. A missing file is `undefined` (the core check reports it). */
export async function readJson(
  ctx: Context,
  rel: string,
): Promise<Record<string, unknown> | undefined> {
  const text = await read(ctx, rel);
  if (text === undefined) return undefined;
  try {
    const parsed: unknown = JSON.parse(text);
    if (isObject(parsed)) return parsed;
    problem(ctx, rel, 'must be a JSON object');
  } catch (error) {
    problem(ctx, rel, `is not valid JSON (${(error as Error).message})`);
  }
  return undefined;
}

interface Frontmatter {
  readonly data: Record<string, unknown> | undefined;
  readonly body: string;
  readonly error: string | undefined;
}

/** Splits `---` frontmatter from the body and parses it as YAML (what the tools do). */
export function frontmatter(text: string): Frontmatter {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/.exec(text);
  if (match === null) return { data: undefined, body: text, error: undefined };
  const body = text.slice(match[0].length);
  try {
    const parsed: unknown = Bun.YAML.parse(match[1] ?? '');
    if (isObject(parsed)) return { data: parsed, body, error: undefined };
    return { data: undefined, body, error: 'frontmatter must be a YAML mapping' };
  } catch (error) {
    const reason = (error as Error).message;
    return { data: undefined, body, error: `frontmatter is not valid YAML (${reason})` };
  }
}

/** `paths:` and `globs:` come as a list or as one comma separated string. */
export function patternsOf(value: unknown): string[] {
  if (typeof value === 'string') {
    return value
      .split(',')
      .map((part) => part.trim())
      .filter(Boolean);
  }
  return Array.isArray(value) ? value.filter(nonEmptyString) : [];
}

const lineCount = (text: string): number => text.replace(/\r?\n$/, '').split(/\r?\n/).length;

/** Problem if `text` (the content of `rel`) has more than `max` lines. */
export function checkLines(ctx: Context, rel: string, text: string, max: number): void {
  const lines = lineCount(text);
  if (lines > max) problem(ctx, rel, `${lines} lines, budget is ${max}`);
}

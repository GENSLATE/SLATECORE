// cspell:words tera
/**
 * A tiny renderer for `scripts/templates/*`, used by `new-app`. It supports the subset of Tera
 * the templates use:
 *   `{{ var }}`, `{{ var + 1 }}`, `{{ var | replace(from="-", to="_") }}`, `upper`, `lower`.
 * Files ending in `.tera` are rendered and lose the suffix; every other file is copied as is.
 * (Template files keep the `.tera` suffix so linters and compilers never see unrendered tags.)
 */
import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, relative } from 'node:path';

export type TemplateVars = Readonly<Record<string, string | number>>;

const EXPRESSION = /\{\{\s*(.+?)\s*\}\}/g;

export function renderString(source: string, vars: TemplateVars): string {
  return source.replace(EXPRESSION, (_match, expression: string) => evaluate(expression, vars));
}

/** Renders every file of `templateDir` into `destination` and returns the written paths. */
export async function renderTemplate(
  templateDir: string,
  destination: string,
  vars: TemplateVars,
): Promise<string[]> {
  const written: string[] = [];
  for (const file of await listFiles(templateDir)) {
    const rel = relative(templateDir, file);
    const rendered = rel.endsWith('.tera');
    const target = join(destination, rendered ? rel.slice(0, -'.tera'.length) : rel);
    const content = await readFile(file);
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, rendered ? renderString(content.toString('utf8'), vars) : content);
    written.push(target);
  }
  return written;
}

function evaluate(expression: string, vars: TemplateVars): string {
  const [head = '', ...filters] = expression.split('|').map((part) => part.trim());
  let value: string | number;
  const arithmetic = /^(\w+)\s*([+-])\s*(\d+)$/.exec(head);
  if (arithmetic !== null) {
    const [, key = '', operator, amount = '0'] = arithmetic;
    const base = Number(lookup(key, vars));
    value = operator === '+' ? base + Number(amount) : base - Number(amount);
  } else {
    value = lookup(head, vars);
  }
  for (const filter of filters) value = applyFilter(filter, String(value));
  return String(value);
}

function applyFilter(filter: string, value: string): string {
  const replace = /^replace\(\s*from\s*=\s*"([^"]*)"\s*,\s*to\s*=\s*"([^"]*)"\s*\)$/.exec(filter);
  if (replace !== null) return value.replaceAll(replace[1] ?? '', replace[2] ?? '');
  if (filter === 'upper') return value.toUpperCase();
  if (filter === 'lower') return value.toLowerCase();
  throw new Error(`template: unsupported filter "${filter}"`);
}

function lookup(key: string, vars: TemplateVars): string | number {
  const value = vars[key];
  if (value === undefined) throw new Error(`template: unknown variable "${key}"`);
  return value;
}

async function listFiles(dir: string): Promise<string[]> {
  const entries = await readdir(dir, { withFileTypes: true, recursive: true });
  return entries
    .filter((entry) => entry.isFile())
    .map((entry) => join(entry.parentPath, entry.name))
    .sort();
}

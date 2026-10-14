/**
 * `bun run new-app` logic: render the `scripts/templates` into `programs/desktop/<name>` and
 * `crates/<name>-core`, then register the new crates in the root `Cargo.toml`. Everything takes
 * the repo root as a parameter so tests can run against a temporary copy.
 */
import { readdir } from 'node:fs/promises';
import { join } from 'node:path';

import { renderTemplate, type TemplateVars } from './template';

const NAME = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

/** Dev ports in use by design: launcher 1420, Design Kit 1430 (each also takes port + 1 for HMR). */
const RESERVED_PORTS: readonly number[] = [1420, 1430];
const FIRST_APP_PORT = 1440;

export interface NewAppOptions {
  /** Kebab-case id, for example `notes`. */
  readonly name: string;
  /** Display name without the suite prefix. Default: the title-cased `name`. */
  readonly title?: string;
  /** One line, no trailing period. */
  readonly description?: string;
  /** Bundle identifier. Default `xyz.genslate.slatecore.<name without dashes>`. */
  readonly identifier?: string;
  /** Vite port (HMR is port + 1). Default: the next free pair. */
  readonly port?: number;
}

export interface NewAppResult {
  readonly appDir: string;
  readonly coreDir: string;
  readonly vars: TemplateVars;
  /** Where the placeholder icon was written. */
  readonly iconPath: string;
  readonly written: readonly string[];
}

export function isAppName(name: string): boolean {
  return NAME.test(name);
}

/** `code-review` → `Code Review`. */
export function titleCase(name: string): string {
  return name
    .split('-')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

async function listDir(dir: string): Promise<string[]> {
  try {
    return await readdir(dir);
  } catch {
    return [];
  }
}

async function readJson(path: string): Promise<Record<string, unknown> | undefined> {
  const file = Bun.file(path);
  if (!(await file.exists())) return undefined;
  try {
    return (await file.json()) as Record<string, unknown>;
  } catch {
    return undefined;
  }
}

/** Dev-server ports already claimed by desktop apps (`build.devUrl`) plus the reserved ones. */
export async function usedPorts(root: string): Promise<number[]> {
  const ports = new Set<number>(RESERVED_PORTS);
  for (const app of await listDir(join(root, 'programs/desktop'))) {
    const conf = await readJson(join(root, 'programs/desktop', app, 'src-tauri/tauri.conf.json'));
    const build = conf?.['build'];
    const devUrl =
      typeof build === 'object' && build !== null && 'devUrl' in build ? build.devUrl : undefined;
    if (typeof devUrl !== 'string') continue;
    try {
      const port = Number(new URL(devUrl).port);
      if (Number.isInteger(port) && port > 0) ports.add(port);
    } catch {
      // A devUrl that is not a URL claims no port.
    }
  }
  return [...ports].sort((a, b) => a - b);
}

/** True when `port` or its HMR neighbour collides with a used port or its HMR neighbour. */
export function portConflicts(port: number, used: readonly number[]): boolean {
  return used.some((other) => Math.abs(other - port) <= 1);
}

/** The first free `port, port + 1` pair from 1440 upwards, in steps of 10. */
export function nextPort(used: readonly number[]): number {
  let port = FIRST_APP_PORT;
  while (portConflicts(port, used)) port += 10;
  return port;
}

/** Adds `crate = { path = "..." }` to `[workspace.dependencies]` in alphabetical `genslate-*` order. */
export function addWorkspaceDependency(cargoToml: string, crate: string, path: string): string {
  const lines = cargoToml.split('\n');
  if (lines.some((line) => line.replace(/\s/g, '').startsWith(`${crate}=`))) return cargoToml;
  const table = lines.findIndex((line) => line.trim() === '[workspace.dependencies]');
  if (table === -1) throw new Error('Cargo.toml has no [workspace.dependencies] table');
  let end = lines.findIndex((line, index) => index > table && /^\[/.test(line.trim()));
  if (end === -1) end = lines.length;

  let insertAt = -1;
  for (let index = table + 1; index < end; index++) {
    const key = /^(genslate-[\w-]+)\s*=/.exec(lines[index] ?? '')?.[1];
    if (key === undefined) continue;
    if (key.localeCompare(crate) > 0) {
      insertAt = index;
      break;
    }
    insertAt = index + 1;
  }
  if (insertAt === -1) {
    insertAt = end;
    while (insertAt > table + 1 && lines[insertAt - 1]?.trim() === '') insertAt--;
  }
  lines.splice(insertAt, 0, `${crate} = { path = "${path}" }`);
  return lines.join('\n');
}

/** Adds `member` to the multi-line `members = [ ... ]` array of `[workspace]`, keeping it sorted. */
export function addWorkspaceMember(cargoToml: string, member: string): string {
  const lines = cargoToml.split('\n');
  const start = lines.findIndex((line) => /^members\s*=\s*\[\s*$/.test(line.trim()));
  if (start === -1) throw new Error('Cargo.toml needs a multi-line `members = [` array');
  const end = lines.findIndex((line, index) => index > start && line.trim() === ']');
  if (end === -1) throw new Error('Cargo.toml members array is not closed');
  const members = lines
    .slice(start + 1, end)
    .map((line) => /"([^"]+)"/.exec(line)?.[1])
    .filter((entry): entry is string => entry !== undefined);
  if (members.includes(member)) return cargoToml;
  members.push(member);
  members.sort();
  lines.splice(start + 1, end - start - 1, ...members.map((entry) => `    "${entry}",`));
  return lines.join('\n');
}

/** A neutral Nord app icon (an app window with a Frost accent) until the real glyph is drawn. */
export function placeholderIcon(title: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <title>${title}</title>
  <!--
    Placeholder: a flat Polar Night plate (nord1, nord2 hairline rim), Snow Storm line work and one
    nord8 accent. Replace the glyph with one that says what the app does, then run
    \`bun run tauri icon src/assets/app-icon.svg\` in this folder to regenerate src-tauri/icons.
  -->
  <rect x="100" y="100" width="824" height="824" rx="185" fill="#3b4252"/>
  <rect x="102" y="102" width="820" height="820" rx="183" fill="none" stroke="#434c5e" stroke-width="4"/>
  <path d="M340 316H684Q744 316 744 376V648Q744 708 684 708H340Q280 708 280 648V376Q280 316 340 316Z" fill="none" stroke="#eceff4" stroke-width="40" stroke-linejoin="round"/>
  <path d="M280 420H744" fill="none" stroke="#eceff4" stroke-width="40" opacity="0.4"/>
  <path d="M340 368a16 16 0 1 0 32 0a16 16 0 1 0 -32 0Z" fill="#88c0d0"/>
</svg>
`;
}

async function isEmptyOrMissing(dir: string): Promise<boolean> {
  return (await listDir(dir)).every((entry) => entry === '.gitkeep');
}

/** Package names (`@genslate/*`) already used by workspace packages. */
async function packageNames(root: string): Promise<Set<string>> {
  const names = new Set<string>();
  for (const parent of ['programs/desktop', 'programs/webapp', 'packages']) {
    for (const entry of await listDir(join(root, parent))) {
      const pkg = await readJson(join(root, parent, entry, 'package.json'));
      if (typeof pkg?.['name'] === 'string') names.add(pkg['name']);
    }
  }
  return names;
}

/**
 * Renders both templates for `options.name` under `root`, registers the new crates in the root
 * `Cargo.toml` and writes the placeholder icon. Does not run `bun install` or generate bundle icons.
 */
export async function scaffoldApp(
  root: string,
  templatesDir: string,
  options: NewAppOptions,
): Promise<NewAppResult> {
  const { name } = options;
  if (!isAppName(name)) {
    throw new Error(`"${name}" must be kebab-case (for example "notes" or "code-review")`);
  }
  const appDir = join(root, 'programs/desktop', name);
  const coreDir = join(root, 'crates', `${name}-core`);
  for (const dir of [appDir, coreDir]) {
    if (!(await isEmptyOrMissing(dir))) throw new Error(`${dir} already exists and is not empty`);
  }
  if ((await packageNames(root)).has(`@genslate/${name}`)) {
    throw new Error(`a workspace package named @genslate/${name} already exists`);
  }

  const used = await usedPorts(root);
  const port = options.port ?? nextPort(used);
  if (!Number.isInteger(port) || port < 1024 || port > 65_534) {
    throw new Error(`invalid port "${port}"`);
  }
  if (portConflicts(port, used)) {
    throw new Error(`port ${port} (or its HMR port ${port + 1}) is used by another app`);
  }

  const display = (options.title ?? titleCase(name)).replace(/^SLATECORE\s+/i, '').trim();
  const title = `SLATECORE ${display}`;
  const vars: TemplateVars = {
    name,
    snake: name.replaceAll('-', '_'),
    display,
    title,
    identifier: options.identifier ?? `xyz.genslate.slatecore.${name.replaceAll('-', '')}`,
    port,
    description: (options.description ?? `${title} desktop app`).replace(/\.$/, ''),
  };

  const written = [
    ...(await renderTemplate(join(templatesDir, 'tauri-app'), appDir, vars)),
    ...(await renderTemplate(join(templatesDir, 'app-core'), coreDir, vars)),
  ];

  const iconPath = join(appDir, 'src/assets/app-icon.svg');
  await Bun.write(iconPath, placeholderIcon(title));
  written.push(iconPath);

  const cargoPath = join(root, 'Cargo.toml');
  let cargo = await Bun.file(cargoPath).text();
  cargo = addWorkspaceMember(cargo, `crates/${name}-core`);
  cargo = addWorkspaceMember(cargo, `programs/desktop/${name}/src-tauri`);
  cargo = addWorkspaceDependency(cargo, `genslate-${name}-core`, `crates/${name}-core`);
  await Bun.write(cargoPath, cargo);

  return { appDir, coreDir, vars, iconPath, written };
}

/**
 * Tier 3 of the portability test (nightly): compare a Process Monitor trace of the launcher with
 * `allowlist.json`. Every successful write that the launcher, or a process it started, makes
 * outside installDir must be explained by an allowlist rule, or the check fails.
 *
 * Capturing (on a Windows PC, as administrator; Process Monitor is a Sysinternals download and is
 * needed only for this step, never for the other tests):
 *
 *     Procmon64.exe /AcceptEula /Quiet /Minimized /BackingFile trace.pml
 *     ... start the packaged slatecore-launcher.exe, use it, quit it ...
 *     Procmon64.exe /Terminate
 *     Procmon64.exe /OpenLog trace.pml /SaveAs trace.csv
 *
 * Format read here: the CSV Process Monitor saves with its default columns, UTF-8 (with or
 * without a byte order mark), a header line, fields in double quotes, a double quote inside a
 * field written as two:
 *
 *     "Time of Day","Process Name","PID","Operation","Path","Result","Detail"
 *
 * Check it:  bun scripts/no-trace/procmon.ts --csv trace.csv --install-dir <unpacked install folder>
 *            [--allowlist <file>]   (or --pml trace.pml with PROCMON_PATH set to convert first)
 * Exit codes: 0 clean, 1 violations or an empty trace, 2 usage.
 *
 * UNVERIFIED on a real trace: the column set and the `Detail` wording of `CreateFile` and
 * `Process Create` follow Process Monitor's documented output; the first nightly run on Windows
 * will show whether the allowlist needs more rules.
 */
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { parseArgs } from 'node:util';

/** One line of the trace. */
export interface ProcmonRow {
  readonly time: string;
  readonly process: string;
  readonly pid: number;
  readonly operation: string;
  readonly path: string;
  readonly result: string;
  readonly detail: string;
}

/** Splits CSV text into records of fields (RFC 4180: quotes, doubled quotes, newlines inside). */
function parseCsv(text: string): string[][] {
  const records: string[][] = [];
  let record: string[] = [];
  let field = '';
  let quoted = false;
  const source = text.replace(/^﻿/, '');
  for (let index = 0; index < source.length; index++) {
    const char = source[index];
    if (quoted) {
      if (char === '"' && source[index + 1] === '"') {
        field += '"';
        index++;
      } else if (char === '"') {
        quoted = false;
      } else {
        field += char;
      }
    } else if (char === '"') {
      quoted = true;
    } else if (char === ',') {
      record.push(field);
      field = '';
    } else if (char === '\n' || char === '\r') {
      if (char === '\r' && source[index + 1] === '\n') index++;
      record.push(field);
      field = '';
      if (record.length > 1 || record[0] !== '') records.push(record);
      record = [];
    } else {
      field += char;
    }
  }
  record.push(field);
  if (record.length > 1 || record[0] !== '') records.push(record);
  return records;
}

const COLUMNS = ['Time of Day', 'Process Name', 'PID', 'Operation', 'Path', 'Result', 'Detail'];

/** Parses a Process Monitor CSV export (see the format above). */
export function parseProcmonCsv(text: string): ProcmonRow[] {
  const [header, ...records] = parseCsv(text);
  if (header === undefined) throw new Error('the trace is empty');
  const index = COLUMNS.map((name) => header.indexOf(name));
  const missing = COLUMNS.filter((_, position) => index[position] === -1);
  if (missing.length > 0) {
    throw new Error(
      `the trace has no column ${missing.join(', ')}; export with the default columns`,
    );
  }
  const at = (record: string[], position: number): string => record[index[position] ?? -1] ?? '';
  return records.map((record) => ({
    time: at(record, 0),
    process: at(record, 1),
    pid: Number(at(record, 2)),
    operation: at(record, 3),
    path: at(record, 4),
    result: at(record, 5),
    detail: at(record, 6),
  }));
}

/** One allowed place outside installDir, or installDir itself. */
export interface AllowRule {
  readonly id: string;
  readonly reason: string;
  /** Glob over the whole path: `*` any text without `\`, `**` any text. Case-insensitive.
   * `{installDir}` and `%VARIABLE%` are expanded first. */
  readonly path: string;
}

export interface Allowlist {
  readonly schema: 1;
  /** The program whose process tree is judged. */
  readonly root: string;
  readonly rules: readonly AllowRule[];
}

/** Validates the parsed `allowlist.json`. */
export function parseAllowlist(value: unknown): Allowlist {
  const raw = value as { schema?: unknown; root?: unknown; rules?: unknown };
  if (raw.schema !== 1) throw new Error('allowlist: schema must be 1');
  if (typeof raw.root !== 'string' || raw.root === '')
    throw new Error('allowlist: root is missing');
  if (!Array.isArray(raw.rules)) throw new Error('allowlist: rules must be a list');
  const seen = new Set<string>();
  const rules = (raw.rules as Record<string, unknown>[]).map((rule) => {
    const { id, reason, path } = rule;
    if (typeof id !== 'string' || typeof reason !== 'string' || typeof path !== 'string') {
      throw new Error('allowlist: every rule needs a string id, reason and path');
    }
    if (seen.has(id)) throw new Error(`allowlist: duplicate rule id "${id}"`);
    seen.add(id);
    if (reason.trim().length < 10) throw new Error(`allowlist: rule "${id}" needs a real reason`);
    if (!/^(\{installDir\}|%[A-Za-z]+%|\\Device\\|HK[A-Z]{1,3}\\)/.test(path)) {
      throw new Error(
        `allowlist: rule "${id}" must start with {installDir}, %VARIABLE%, \\Device\\ or a registry hive`,
      );
    }
    return { id, reason, path };
  });
  return { schema: 1, root: raw.root, rules };
}

/** Expands `{installDir}` and `%NAME%` in a rule path; a variable that is not set is an error. */
function expand(
  path: string,
  installDir: string,
  env: Readonly<Record<string, string | undefined>>,
): string {
  return path
    .replace('{installDir}', installDir.replace(/[\\/]+$/, ''))
    .replace(/%([A-Za-z]+)%/g, (_, name: string) => {
      const found = Object.entries(env).find(([key]) => key.toLowerCase() === name.toLowerCase());
      if (found?.[1] === undefined) throw new Error(`allowlist uses %${name}% but it is not set`);
      return found[1].replace(/[\\/]+$/, '');
    });
}

/** A case-insensitive matcher for a glob over a whole Windows path. */
export function globToRegExp(glob: string): RegExp {
  let source = '';
  for (let index = 0; index < glob.length; index++) {
    const char = glob[index] ?? '';
    if (char === '*' && glob[index + 1] === '*') {
      source += '.*';
      index++;
    } else if (char === '*') {
      source += '[^\\\\]*';
    } else {
      source += char.replace(/[.+?^${}()|[\]\\/]/g, '\\$&');
    }
  }
  return new RegExp(`^${source}$`, 'i');
}

const WRITE_OPERATIONS = new Set([
  'writefile',
  'setendoffileinformationfile',
  'setallocationinformationfile',
  'setrenameinformationfile',
  'setdispositioninformationfile',
  'regsetvalue',
  'regdeletekey',
  'regdeletevalue',
  'regrenamekey',
  'regsetkeysecurity',
]);

/** Whether a row changes something: a write, a rename, a delete, a created file or key. */
export function isWrite(row: ProcmonRow): boolean {
  if (row.result.toUpperCase() !== 'SUCCESS') return false;
  const operation = row.operation.toLowerCase();
  if (WRITE_OPERATIONS.has(operation)) return true;
  if (operation === 'createfile') {
    return (
      /OpenResult: *(Created|Overwritten|Superseded)/i.test(row.detail) ||
      /Desired Access:[^,]*\b(Write|Append|Delete|All Access)/i.test(row.detail)
    );
  }
  if (operation === 'regcreatekey') return /REG_CREATED_NEW_KEY/i.test(row.detail);
  return false;
}

/** PIDs of the root program and of every process created, directly or not, by one of them. */
export function launcherProcessIds(rows: readonly ProcmonRow[], root: string): Set<number> {
  const tree = new Set<number>();
  for (const row of rows) {
    if (row.process.toLowerCase() === root.toLowerCase()) tree.add(row.pid);
    if (row.operation === 'Process Create' && tree.has(row.pid)) {
      const child = /PID:\s*(\d+)/.exec(row.detail)?.[1];
      if (child !== undefined) tree.add(Number(child));
    }
  }
  return tree;
}

export interface TraceReport {
  /** Events of the launcher's process tree. */
  readonly events: number;
  /** Of those, the writes. */
  readonly writes: number;
  /** Writes no rule explains. */
  readonly violations: readonly ProcmonRow[];
  /** How many writes each rule explained. */
  readonly allowed: Readonly<Record<string, number>>;
}

/** Judges a trace. A write is fine when a rule matches its path (installDir is a rule). */
export function checkTrace(
  rows: readonly ProcmonRow[],
  options: {
    readonly installDir: string;
    readonly allowlist: Allowlist;
    readonly env: Readonly<Record<string, string | undefined>>;
  },
): TraceReport {
  const tree = launcherProcessIds(rows, options.allowlist.root);
  const rules = options.allowlist.rules.map((rule) => ({
    id: rule.id,
    match: globToRegExp(expand(rule.path, options.installDir, options.env)),
  }));
  const own = rows.filter((row) => tree.has(row.pid));
  const writes = own.filter(isWrite);
  const allowed: Record<string, number> = {};
  const violations: ProcmonRow[] = [];
  for (const row of writes) {
    const rule = rules.find(({ match }) => match.test(row.path));
    if (rule === undefined) violations.push(row);
    else allowed[rule.id] = (allowed[rule.id] ?? 0) + 1;
  }
  return { events: own.length, writes: writes.length, violations, allowed };
}

/** The arguments that convert a `.pml` log to CSV with Process Monitor. */
export function pmlToCsvArgs(procmon: string, pml: string, csv: string): string[] {
  return [procmon, '/AcceptEula', '/Quiet', '/OpenLog', pml, '/SaveAs', csv];
}

async function main(): Promise<number> {
  const { values } = parseArgs({
    args: Bun.argv.slice(2),
    options: {
      csv: { type: 'string' },
      pml: { type: 'string' },
      'install-dir': { type: 'string' },
      allowlist: { type: 'string' },
    },
    strict: true,
  });
  const installDir = values['install-dir'];
  let csv = values.csv;
  if (installDir === undefined || (csv === undefined && values.pml === undefined)) {
    console.error(
      'usage: bun scripts/no-trace/procmon.ts --csv <trace.csv> --install-dir <folder> [--allowlist <file>]\n       (or --pml <trace.pml> with PROCMON_PATH set)',
    );
    return 2;
  }
  if (csv === undefined && values.pml !== undefined) {
    const procmon = process.env['PROCMON_PATH'];
    if (procmon === undefined || procmon === '') {
      console.error(
        '--pml needs PROCMON_PATH (the path of Procmon64.exe); or export a CSV yourself',
      );
      return 2;
    }
    csv = `${values.pml}.csv`;
    const child = Bun.spawn(pmlToCsvArgs(procmon, resolve(values.pml), resolve(csv)), {
      stdout: 'inherit',
      stderr: 'inherit',
    });
    if ((await child.exited) !== 0) {
      console.error('Process Monitor could not convert the log');
      return 2;
    }
  }
  const allowlistPath = values.allowlist ?? resolve(dirname(import.meta.path), 'allowlist.json');
  const allowlist = parseAllowlist(JSON.parse(await readFile(allowlistPath, 'utf8')));
  const rows = parseProcmonCsv(await readFile(csv ?? '', 'utf8'));
  const report = checkTrace(rows, { installDir, allowlist, env: process.env });

  console.log(
    `${report.events} events of ${allowlist.root} and its children, ${report.writes} writes`,
  );
  for (const [id, count] of Object.entries(report.allowed))
    console.log(`  allowed by ${id}: ${count}`);
  if (report.events === 0) {
    console.error(`no events from ${allowlist.root} in the trace: nothing was checked`);
    return 1;
  }
  for (const row of report.violations) {
    console.error(`VIOLATION ${row.process} (${row.pid}) ${row.operation} ${row.path}`);
  }
  return report.violations.length === 0 ? 0 : 1;
}

if (import.meta.main) process.exit(await main());

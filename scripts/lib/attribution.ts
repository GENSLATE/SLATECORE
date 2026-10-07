/**
 * Authorship guard: SLATECORE code is authored by GENSLATE. Agent tooling may be used to write
 * it (`.claude/`, `CLAUDE.md`, `.cursor/`), but no commit, branch or PR may credit an agent as
 * its author. These pure checks back `bun run attribution` (commit-msg and pre-push hooks, CI).
 */

/** One reason a commit, branch or PR body was rejected. */
export interface AttributionFinding {
  /** What was checked, e.g. `commit 1a2b3c4`, `branch`, `PR body`. */
  readonly where: string;
  /** Why it was rejected. */
  readonly reason: string;
}

/** A commit as read from `git log`. */
export interface CommitIdentity {
  readonly sha: string;
  readonly authorName: string;
  readonly authorEmail: string;
  readonly committerName: string;
  readonly committerEmail: string;
  readonly message: string;
}

/** Agent and tool names that must never be credited (matched case-insensitively). */
export const AGENT_NAMES: readonly string[] = [
  'claude',
  'anthropic',
  'cursor',
  'cursoragent',
  'gemini',
  'antigravity',
  'copilot',
  'codex',
  'openai',
  'chatgpt',
  'devin',
  'windsurf',
  'aider',
  'jules',
];

/** Branch prefixes agent tools create by default. */
export const AGENT_BRANCH_PREFIXES: readonly string[] = [
  'claude/',
  'cursor/',
  'codex/',
  'copilot/',
  'devin/',
  'jules/',
  'antigravity/',
];

const NAMES = AGENT_NAMES.join('|');

/** An agent name as a whole word (so `Cursory` or `Devine` are not flagged). */
const AGENT_WORD = new RegExp(`(?<![a-z0-9])(?:${NAMES})(?![a-z0-9])`, 'i');

/** `Key: value` git trailers that can carry an author credit. */
const CREDIT_TRAILER = /^(?:co-authored-by|signed-off-by|reviewed-by|authored-by|assisted-by)\s*:/i;

/** A trailer whose key itself names an agent, e.g. `Claude-Session:` or `Cursor-Agent-Id:`. */
const AGENT_KEY_TRAILER = new RegExp(`^(?:[a-z0-9]+-)*(?:${NAMES})(?:-[a-z0-9]+)*\\s*:`, 'i');

/**
 * Footers such as `Generated with [Claude Code]`, `Made with Cursor`, `Created by Devin`, and the
 * hyphenated trailer forms `Made-with: Cursor` and `Generated-by: Claude`.
 */
const FOOTER = new RegExp(
  `^\\W*(?:generated|made|created|written|authored|assisted|built|powered|coauthored|co-authored)[\\s-]+(?:with|by|using)\\b.*(?<![a-z0-9])(?:${NAMES})(?![a-z0-9])`,
  'i',
);

/** Session and share links of the agent tools. */
const SESSION_LINK =
  /(?:claude\.ai\/(?:code|chat)|claude\.com\/claude-code|cursor\.com\/(?:agents|background-agent)|app\.devin\.ai|chatgpt\.com\/(?:codex|s\/)|jules\.google|antigravity\.google|windsurf\.com\/share)/i;

/** The first line of a line-based rule that credits an agent, with the reason it matched. */
function lineReason(line: string): string | undefined {
  const text = line.trim();
  if (CREDIT_TRAILER.test(text) && AGENT_WORD.test(text)) {
    return `a ${text.slice(0, text.indexOf(':'))} trailer credits an agent`;
  }
  if (AGENT_KEY_TRAILER.test(text)) return `an agent trailer (${text.slice(0, text.indexOf(':'))})`;
  if (FOOTER.test(text)) return 'a "Generated with/by" style agent footer';
  if (SESSION_LINK.test(text)) return 'an agent session link';
  return undefined;
}

/** The line `git commit -v` puts before the staged diff; everything after it is not the message. */
const SCISSORS = /^# -{10,} >8 -{10,}/;

/** Splits message lines at the scissors line: the message and the untouched rest (the diff). */
function splitAtScissors(lines: readonly string[]): {
  readonly message: string[];
  readonly rest: string[];
} {
  const index = lines.findIndex((line) => SCISSORS.test(line));
  return index === -1
    ? { message: [...lines], rest: [] }
    : { message: lines.slice(0, index), rest: lines.slice(index) };
}

function splitLines(text: string): string[] {
  return text.replaceAll('\r\n', '\n').split('\n');
}

function isComment(line: string): boolean {
  return line.startsWith('#');
}

/** Findings for a commit message or PR body. Plain mentions of the `.claude/` tooling pass. */
export function checkText(where: string, text: string): AttributionFinding[] {
  const reasons = new Set<string>();
  // Comments (`# ...`) are stripped by git, and nothing after the scissors line is the message.
  for (const line of splitAtScissors(splitLines(text)).message) {
    if (isComment(line)) continue;
    const reason = lineReason(line);
    if (reason !== undefined) reasons.add(reason);
  }
  return [...reasons].map((reason) => ({ where, reason }));
}

/**
 * Removes every line `checkText` rejects (plus the blank lines they leave at the end), keeping
 * everything else, including a `git commit -v` diff below the scissors line, byte for byte.
 * Returns the message unchanged when it is already clean.
 */
export function stripAgentCredit(text: string): string {
  const eol = text.includes('\r\n') ? '\r\n' : '\n';
  const { message, rest } = splitAtScissors(splitLines(text));
  const kept = message.filter((line) => isComment(line) || lineReason(line) === undefined);
  if (kept.length === message.length) return text;
  while (kept.length > 0 && kept[kept.length - 1]?.trim() === '') kept.pop();
  const head = kept.length === 0 ? '' : `${kept.join(eol)}${eol}`;
  return rest.length === 0 ? head : `${head}${rest.join(eol)}`;
}

const AGENT_EMAIL =
  /(?:@(?:anthropic|openai|cursor|codeium|windsurf)\.com$|^(?:cursoragent|claude|copilot|codex|devin|jules|aider)@)/i;

/** Findings for a git identity (`name <email>`) used as author or committer. */
export function checkIdentity(
  where: string,
  role: 'author' | 'committer',
  name: string,
  email: string,
): AttributionFinding[] {
  const agent = AGENT_WORD.test(name) || AGENT_EMAIL.test(email.trim());
  return agent ? [{ where, reason: `${role} is ${name} <${email}>` }] : [];
}

/** Findings for a branch name (agent tools create `claude/...`, `cursor/...`, `codex/...`). */
export function checkBranch(name: string): AttributionFinding[] {
  const lower = name.trim().toLowerCase();
  return AGENT_BRANCH_PREFIXES.some((prefix) => lower.startsWith(prefix))
    ? [{ where: 'branch', reason: `"${name}" names an agent; rename the branch` }]
    : [];
}

/** Findings for one commit: author, committer and message. */
export function checkCommit(commit: CommitIdentity): AttributionFinding[] {
  const where = `commit ${commit.sha.slice(0, 7)}`;
  return [
    ...checkIdentity(where, 'author', commit.authorName, commit.authorEmail),
    ...checkIdentity(where, 'committer', commit.committerName, commit.committerEmail),
    ...checkText(where, commit.message),
  ];
}

const FIELD = '\u001F';
const RECORD = '\u001E';

/** `git log` format that `parseLog` reads. */
export const LOG_FORMAT = '%H%x1f%an%x1f%ae%x1f%cn%x1f%ce%x1f%B%x1e';

/** Parses `git log --format=LOG_FORMAT` output. */
export function parseLog(output: string): CommitIdentity[] {
  return output
    .split(RECORD)
    .map((record) => record.replace(/^\n/, ''))
    .filter((record) => record.trim() !== '')
    .map((record) => {
      const [
        sha = '',
        authorName = '',
        authorEmail = '',
        committerName = '',
        committerEmail = '',
        message = '',
      ] = record.split(FIELD);
      return { sha, authorName, authorEmail, committerName, committerEmail, message };
    });
}

/** Splits `git var GIT_AUTHOR_IDENT` output (`Name <email> 1700000000 +0000`). */
export function parseIdent(ident: string): { readonly name: string; readonly email: string } {
  const match = /^(.*?)\s*<([^>]*)>/.exec(ident.trim());
  return { name: match?.[1] ?? ident.trim(), email: match?.[2] ?? '' };
}

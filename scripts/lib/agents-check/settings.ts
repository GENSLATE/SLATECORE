/** `.claude/settings.json` (attribution, PowerShell pin, permissions, hooks) and the MCP files. */
import { type Context, exists, isObject, problem, readJson } from './context';

const SETTINGS = '.claude/settings.json';
const fail = (ctx: Context, message: string): void => problem(ctx, SETTINGS, message);
const HOOK_DIR = `\${CLAUDE_PROJECT_DIR}/`; // Claude Code expands it; the check compares the text.

/** Denied so that bun-only, the git hooks and the secrets hold even when the PowerShell tool is on. */
const REQUIRED_DENY: readonly string[] = [
  ...['npm', 'npx', 'pnpm', 'yarn', 'node'].flatMap((tool) => [
    `Bash(${tool} *)`,
    `PowerShell(${tool} *)`,
  ]),
  'Bash(git * --no-verify*)',
  'Bash(git commit -n*)',
  'Bash(*LEFTHOOK=0*)',
  'Read(**/.env)',
  'Read(**/.genslate/**)',
  'Read(**/storage/vault/**)',
];

/** An allow rule that approves every command: `*`, `Bash`, `Bash(*)`, `PowerShell(*)`. */
const ALLOWS_EVERYTHING = /^(?:\*|(?:Bash|PowerShell)(?:\(\*+\))?)$/;

/** A hook handler runs `bun <script>` without a shell, and the script exists. */
async function checkHandler(ctx: Context, where: string, handler: unknown): Promise<void> {
  const command = isObject(handler) ? handler['command'] : undefined;
  const args = isObject(handler) ? handler['args'] : undefined;
  const script = Array.isArray(args) ? args[0] : undefined;
  if (command !== 'bun' || typeof script !== 'string') {
    const example = `"command": "bun" plus "args": ["${HOOK_DIR}<script>"]`;
    fail(ctx, `${where} must use exec form: ${example} (got ${JSON.stringify(command)})`);
  } else if (!script.startsWith(HOOK_DIR)) {
    fail(ctx, `${where} script must start with ${HOOK_DIR}`);
  } else if (!(await exists(ctx, script.slice(HOOK_DIR.length)))) {
    fail(ctx, `${where} runs ${script.slice(HOOK_DIR.length)}, which does not exist`);
  }
}

export async function checkSettings(ctx: Context): Promise<void> {
  const settings = await readJson(ctx, SETTINGS);
  if (settings === undefined) return;

  const { attribution, env, hooks } = settings;
  if (
    !isObject(attribution) ||
    attribution['commit'] !== '' ||
    attribution['pr'] !== '' ||
    attribution['sessionUrl'] !== false
  ) {
    fail(
      ctx,
      'attribution must be {"commit": "", "pr": "", "sessionUrl": false} (no agent credit)',
    );
  }
  if (!isObject(env) || env['CLAUDE_CODE_USE_POWERSHELL_TOOL'] !== '0') {
    fail(ctx, 'env.CLAUDE_CODE_USE_POWERSHELL_TOOL must be "0" so Bash(...) rules apply');
  }

  const permissions: Record<string, unknown> = isObject(settings['permissions'])
    ? settings['permissions']
    : {};
  const rules = (key: string): unknown[] => {
    const value = permissions[key];
    return Array.isArray(value) ? value : [];
  };
  for (const rule of REQUIRED_DENY) {
    if (!rules('deny').includes(rule)) fail(ctx, `permissions.deny must contain "${rule}"`);
  }
  for (const rule of rules('allow')) {
    if (typeof rule === 'string' && ALLOWS_EVERYTHING.test(rule)) {
      fail(ctx, `permissions.allow "${rule}" approves every command; allow narrow rules`);
    }
  }
  if (permissions['defaultMode'] === 'bypassPermissions') {
    fail(ctx, 'permissions.defaultMode must not be bypassPermissions');
  }

  for (const [event, entries] of Object.entries(isObject(hooks) ? hooks : {})) {
    for (const [index, entry] of (Array.isArray(entries) ? entries : []).entries()) {
      const handlers = isObject(entry) && Array.isArray(entry['hooks']) ? entry['hooks'] : [];
      for (const [position, handler] of handlers.entries()) {
        await checkHandler(ctx, `hooks.${event}[${index}].hooks[${position}]`, handler);
      }
    }
  }
}

/** MCP files: valid JSON with `mcpServers`, and header and env values read `${NAME}`. */
export async function checkMcp(ctx: Context): Promise<void> {
  for (const rel of ['.mcp.json', '.cursor/mcp.json', '.agents/mcp_config.json']) {
    const config = await readJson(ctx, rel);
    if (config === undefined) continue;
    const servers = config['mcpServers'];
    if (!isObject(servers)) {
      problem(ctx, rel, 'needs an "mcpServers" object');
      continue;
    }
    for (const [name, server] of Object.entries(servers)) {
      for (const field of ['headers', 'env']) {
        const values = isObject(server) ? server[field] : undefined;
        for (const [key, value] of Object.entries(isObject(values) ? values : {})) {
          if (typeof value !== 'string' || !/\$\{[^}]+\}/.test(value)) {
            const where = `server "${name}" ${field}.${key}`;
            problem(
              ctx,
              rel,
              `${where} must read an environment variable (\${NAME}), never a secret`,
            );
          }
        }
      }
    }
  }
}

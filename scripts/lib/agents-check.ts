/**
 * Agent-folder drift check (`.claude/`, `.cursor/`, `.agents/`, `AGENTS.md`, `CLAUDE.md`).
 * `bun run check` prints every returned problem and fails when there is one. This is a stub that
 * reports nothing; the agent-folders task fills it in.
 */
export async function checkAgents(_root: string): Promise<string[]> {
  return [];
}

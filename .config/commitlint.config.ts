/**
 * Conventional Commits for SLATECORE, run by lefthook's `commit-msg` hook:
 *   bun x --no-install --bun commitlint --config .config/commitlint.config.ts --edit <file>
 *
 * Scopes are workspace package and crate names plus a few repo-wide areas. No agent credit
 * belongs in a commit message; `bun run attribution` enforces that.
 */

const scopes = [
  // JavaScript packages (packages/*)
  'tokens',
  'design-system',
  'tauri-bridge',
  'config-typescript',
  'config-vite',
  // Apps (programs/*)
  'launcher',
  'example',
  // Rust crates (crates/*)
  'crates',
  'paths',
  'design-tokens',
  'launcher-core',
  'vault',
  'testing',
  // Repo areas
  'scripts',
  'turbo',
  'ci',
  'deps',
  'release',
  'agents',
  'docs',
  'repo',
  'config',
  'vscode',
] as const;

const config = {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'scope-enum': [2, 'always', [...scopes]],
    'scope-case': [2, 'always', 'kebab-case'],
    'subject-case': [2, 'never', ['sentence-case', 'start-case', 'pascal-case', 'upper-case']],
    'header-max-length': [2, 'always', 100],
    'body-max-line-length': [0],
  },
  helpUrl: 'https://www.conventionalcommits.org/en/v1.0.0/',
};

export default config;

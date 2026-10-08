/**
 * Guards for the CI setup under `.github/`: Windows only, every action pinned to a commit, no
 * untrusted text in scripts, least-privilege tokens, cargo-deny on a schedule, caches saved from
 * main only. The workflows are YAML files nobody can run locally, so these tests are the check.
 */
// cspell:ignore Swatinem
import { describe, expect, test } from 'bun:test';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

import commitlint from '../../.config/commitlint.config';
import { ROOT } from '../lib/paths';

interface Step {
  readonly name?: string;
  readonly uses?: string;
  readonly run?: string;
  readonly if?: string;
  readonly with?: Readonly<Record<string, unknown>>;
  readonly env?: Readonly<Record<string, unknown>>;
}

type Permissions = string | Readonly<Record<string, string>>;

interface Job {
  readonly uses?: string;
  readonly 'runs-on'?: string;
  readonly needs?: string | readonly string[];
  readonly permissions?: Permissions;
  readonly 'timeout-minutes'?: number;
  readonly steps?: readonly Step[];
}

interface Workflow {
  readonly name?: string;
  readonly on?: unknown;
  readonly permissions?: Permissions;
  readonly jobs?: Readonly<Record<string, Job>>;
}

interface Action {
  readonly runs?: { readonly using?: string; readonly steps?: readonly Step[] };
}

interface Renovate {
  readonly $schema?: string;
  readonly extends?: readonly string[];
  readonly semanticCommits?: string;
  readonly semanticCommitScope?: string;
  readonly packageRules?: readonly { readonly semanticCommitScope?: string }[];
}

const WORKFLOW_DIR = '.github/workflows';
const CI = `${WORKFLOW_DIR}/ci.yml`;
const SECURITY = `${WORKFLOW_DIR}/security.yml`;
const AUTHORSHIP = `${WORKFLOW_DIR}/authorship.yml`;
const RELEASE = `${WORKFLOW_DIR}/release.yml`;
const SETUP_ENV = '.github/actions/setup-env/action.yml';

const readText = (path: string): string => readFileSync(join(ROOT, path), 'utf8');
const lines = (path: string): string[] => readText(path).split(/\r?\n/);
const parseYaml = <T>(path: string): T => (Bun.YAML.parse(readText(path)) ?? {}) as T;

/** Repo-relative paths (forward slashes) of every file below `dir`. */
function filesBelow(dir: string): string[] {
  const absolute = join(ROOT, dir);
  if (!existsSync(absolute)) return [];
  return readdirSync(absolute, { recursive: true, encoding: 'utf8' })
    .map((entry) => `${dir}/${entry.replaceAll('\\', '/')}`)
    .filter((path) => statSync(join(ROOT, path)).isFile())
    .sort();
}

const WORKFLOWS = filesBelow(WORKFLOW_DIR).filter((path) => path.endsWith('.yml'));
const ACTIONS = filesBelow('.github/actions').filter((path) => path.endsWith('/action.yml'));
const YAML_FILES = [...WORKFLOWS, ...ACTIONS];

const stripComment = (line: string): string => line.replace(/(^|\s)#.*$/, '');

/** A line's `uses:` value and its trailing comment, when the line is a `uses:` entry. */
function usesOf(line: string): { ref: string; comment: string } | undefined {
  const match = /^\s*(?:-\s+)?uses:\s*(\S+)(?:\s+#\s*(.*))?$/.exec(line);
  return match?.[1] === undefined ? undefined : { ref: match[1], comment: match[2] ?? '' };
}

function jobsOf(path: string): [string, Job][] {
  return Object.entries(parseYaml<Workflow>(path).jobs ?? {});
}

/** Steps of a job with the steps of every local composite action inlined behind its `uses`. */
function expandedSteps(job: Job): Step[] {
  return (job.steps ?? []).flatMap((step) => {
    if (step.uses === undefined || !step.uses.startsWith('./.github/actions/')) return [step];
    const action = parseYaml<Action>(`${step.uses.slice(2)}/action.yml`);
    return [step, ...(action.runs?.steps ?? [])];
  });
}

const runsOf = (steps: readonly Step[]): string[] =>
  steps.flatMap((step) => (step.run === undefined ? [] : [step.run]));

/** Every `run:` script of a workflow or composite action file. */
function scriptsOf(path: string): string[] {
  const doc = parseYaml<Workflow & Action>(path);
  const steps = [
    ...(doc.runs?.steps ?? []),
    ...Object.values(doc.jobs ?? {}).flatMap((j) => j.steps ?? []),
  ];
  return runsOf(steps);
}

const hasInstall = (script: string): boolean => script.includes('bun install --frozen-lockfile');
const usesBun = (script: string): boolean => /(^|\s)bun\s+(run|x|test)\b/.test(script);

/** True when a workflow installs with the frozen lockfile itself or through a reusable workflow. */
function installs(path: string, seen: readonly string[] = []): boolean {
  if (seen.includes(path)) return false;
  return jobsOf(path).some(([, job]) =>
    job.uses === undefined
      ? runsOf(expandedSteps(job)).some(hasInstall)
      : installs(job.uses.slice(2), [...seen, path]),
  );
}

function triggersOf(path: string): string[] {
  const doc = parseYaml<Workflow & Record<string, unknown>>(path);
  // YAML 1.1 parsers read the key `on` as the boolean `true`.
  const on = doc.on ?? doc['true'];
  if (typeof on === 'string') return [on];
  if (Array.isArray(on)) return on.map(String);
  return on === null || typeof on !== 'object' ? [] : Object.keys(on);
}

describe('workflow set', () => {
  test('exactly_the_four_planned_workflows_exist', () => {
    expect(WORKFLOWS).toEqual([AUTHORSHIP, CI, RELEASE, SECURITY]);
  });

  test('job_names_match_the_plan_interface', () => {
    const names = (path: string): string[] => jobsOf(path).map(([id]) => id);
    expect(names(CI)).toEqual(['check', 'test', 'package']);
    expect(names(AUTHORSHIP)).toEqual(['authorship']);
    expect(names(SECURITY)).toEqual(['security']);
    expect(names(RELEASE)).toEqual(['ci', 'publish']);
  });

  test('every_workflow_has_a_name_and_a_trigger', () => {
    for (const path of WORKFLOWS) {
      expect(parseYaml<Workflow>(path).name).toBeTruthy();
      expect(triggersOf(path).length).toBeGreaterThan(0);
    }
  });

  test('no_empty_files_remain_in_github', () => {
    const empty = filesBelow('.github').filter(
      (path) => !path.endsWith('.gitkeep') && readText(path).trim() === '',
    );
    expect(empty).toEqual([]);
  });
});

describe('windows only', () => {
  test('every_job_runs_on_windows_latest_or_calls_a_local_workflow', () => {
    for (const path of WORKFLOWS) {
      for (const [id, job] of jobsOf(path)) {
        if (job.uses !== undefined) {
          expect(`${path}:${id}:${job.uses}`).toMatch(/:\.\/\.github\/workflows\/[\w.-]+\.yml$/);
        } else {
          expect(`${path}:${id}:${job['runs-on']}`).toBe(`${path}:${id}:windows-latest`);
        }
      }
    }
  });

  test('no_other_operating_system_is_mentioned', () => {
    const found = YAML_FILES.flatMap((path) =>
      lines(path)
        .map(stripComment)
        .filter((line) => /ubuntu|macos|\blinux\b|self-hosted/i.test(line))
        .map((line) => `${path}: ${line.trim()}`),
    );
    expect(found).toEqual([]);
  });

  test('every_job_has_a_timeout', () => {
    for (const path of WORKFLOWS) {
      for (const [id, job] of jobsOf(path)) {
        if (job.uses !== undefined) continue;
        expect(`${path}:${id}:${typeof job['timeout-minutes']}`).toBe(`${path}:${id}:number`);
      }
    }
  });
});

describe('action pinning', () => {
  test('every_uses_is_a_full_commit_sha_with_a_version_comment', () => {
    const problems: string[] = [];
    for (const path of YAML_FILES) {
      lines(path).forEach((line, index) => {
        const entry = usesOf(line);
        if (entry === undefined) return;
        const where = `${path}:${index + 1}`;
        if (entry.ref.startsWith('./')) {
          if (!entry.ref.startsWith('./.github/') || entry.ref.includes('@'))
            problems.push(`${where}: local uses must stay inside ./.github/ and carry no ref`);
          return;
        }
        if (!/^[\w.-]+\/[\w.-]+(?:\/[\w./-]+)?@[0-9a-f]{40}$/.test(entry.ref))
          problems.push(`${where}: ${entry.ref} is not pinned to a full 40-char commit SHA`);
        else if (!/^v\d+\.\d+\.\d+$/.test(entry.comment.trim()))
          problems.push(`${where}: ${entry.ref} needs a trailing "# vX.Y.Z" comment`);
      });
    }
    expect(problems).toEqual([]);
  });

  test('the_uses_scan_sees_every_uses_the_yaml_parser_sees', () => {
    for (const path of YAML_FILES) {
      const doc = parseYaml<Workflow & Action>(path);
      const parsed = [
        ...(doc.runs?.steps ?? []),
        ...Object.values(doc.jobs ?? {}).flatMap((job) => [job, ...(job.steps ?? [])]),
      ].flatMap((entry) => (entry.uses === undefined ? [] : [entry.uses]));
      const scanned = lines(path).flatMap((line) => usesOf(line)?.ref ?? []);
      expect(scanned.sort()).toEqual(parsed.sort());
    }
  });
});

describe('scripts', () => {
  test('every_job_installs_with_the_frozen_lockfile_before_it_runs_bun', () => {
    for (const path of WORKFLOWS) {
      expect(`${path}:${installs(path)}`).toBe(`${path}:true`);
      for (const [id, job] of jobsOf(path)) {
        if (job.uses !== undefined) continue;
        const scripts = runsOf(expandedSteps(job));
        const first = scripts.findIndex(usesBun);
        if (first === -1) continue;
        const install = scripts.findIndex(hasInstall);
        expect(`${path}:${id}:${install >= 0 && install < first}`).toBe(`${path}:${id}:true`);
      }
    }
  });

  test('bun_install_is_never_run_without_the_frozen_lockfile_flag', () => {
    const loose = YAML_FILES.flatMap(scriptsOf).filter((s) =>
      /bun install(?!\s+--frozen-lockfile)/.test(s),
    );
    expect(loose).toEqual([]);
  });

  test('scripts_use_bun_only', () => {
    const foreign = YAML_FILES.flatMap(scriptsOf).filter((s) =>
      /(^|[\s;&|(])(npm|npx|pnpm|yarn|node)(\s|$)/.test(s),
    );
    expect(foreign).toEqual([]);
  });

  test('scripts_never_interpolate_expressions', () => {
    // Anything from the event (PR title, body, branch name) must reach a script as an env var.
    const interpolated = YAML_FILES.flatMap((path) =>
      scriptsOf(path)
        .filter((script) => script.includes('${{'))
        .map((script) => `${path}: ${script.trim()}`),
    );
    expect(interpolated).toEqual([]);
  });

  test('pull_request_target_is_never_used', () => {
    for (const path of WORKFLOWS) expect(triggersOf(path)).not.toContain('pull_request_target');
  });

  test('checkout_never_persists_credentials', () => {
    const checkouts = WORKFLOWS.flatMap((path) =>
      jobsOf(path).flatMap(([id, job]) =>
        (job.steps ?? [])
          .filter((step) => step.uses?.startsWith('actions/checkout@'))
          .map((step) => `${path}:${id}:${String(step.with?.['persist-credentials'])}`),
      ),
    );
    expect(checkouts.length).toBeGreaterThan(0);
    for (const entry of checkouts) expect(entry.endsWith(':false')).toBe(true);
  });
});

describe('permissions', () => {
  test('every_workflow_declares_read_only_top_level_permissions', () => {
    for (const path of WORKFLOWS) {
      const { permissions } = parseYaml<Workflow>(path);
      expect(typeof permissions).toBe('object');
      expect(Object.values(permissions ?? {}).filter((level) => level !== 'read')).toEqual([]);
    }
  });

  test('only_the_release_publish_job_may_write_and_only_contents', () => {
    const writes = WORKFLOWS.flatMap((path) =>
      jobsOf(path).flatMap(([id, job]) =>
        Object.entries(typeof job.permissions === 'object' ? job.permissions : {})
          .filter(([, level]) => level === 'write')
          .map(([scope]) => `${path}:${id}:${scope}`),
      ),
    );
    expect(writes).toEqual([`${RELEASE}:publish:contents`]);
  });

  test('no_job_uses_a_blanket_permission_string', () => {
    for (const path of WORKFLOWS) {
      for (const [, job] of jobsOf(path)) expect(typeof job.permissions).not.toBe('string');
    }
  });
});

describe('ci workflow', () => {
  const ci = parseYaml<Workflow>(CI);
  const ciScripts = scriptsOf(CI);

  test('runs_on_pushes_to_main_pull_requests_and_when_called_by_release', () => {
    expect(triggersOf(CI).sort()).toEqual(['pull_request', 'push', 'workflow_call']);
  });

  test('check_test_and_package_run_the_root_commands', () => {
    const runs = (job: string): string[] => runsOf(expandedSteps(ci.jobs?.[job] ?? {}));
    expect(runs('check').some((s) => s.includes('bun run check --ts'))).toBe(true);
    expect(runs('test').some((s) => s.includes('bun run test'))).toBe(true);
    expect(runs('package').some((s) => s.includes('bun run package'))).toBe(true);
  });

  test('cargo_deny_never_runs_per_pull_request', () => {
    expect(ciScripts.filter((script) => /deny/i.test(script))).toEqual([]);
    // `bun run check` without --ts would include //#rust:deny (see scripts/commands/check.ts).
    const bareChecks = ciScripts.filter((s) => /bun run check/.test(s) && !s.includes('--ts'));
    expect(bareChecks).toEqual([]);
  });

  test('every_rust_gate_of_bun_run_check_except_deny_still_runs', () => {
    const source = readText('scripts/commands/check.ts');
    const declared = /RUST_TASKS\s*=\s*\[([^\]]*)\]/.exec(source)?.[1] ?? '';
    const gates = [...declared.matchAll(/'(\/\/#[^']+)'/g)].map((match) => match[1] ?? '');
    expect(gates.length).toBeGreaterThan(0);
    const expected = gates.filter((gate) => gate !== '//#rust:deny').sort();
    const turbo = ciScripts.find((script) => /turbo run/.test(script)) ?? '';
    expect([...turbo.matchAll(/\/\/#[\w:-]+/g)].map((match) => match[0]).sort()).toEqual(expected);
  });

  test('turbo_cache_is_saved_from_main_only_and_per_job', () => {
    const saves = Object.entries(ci.jobs ?? {}).flatMap(([id, job]) =>
      (job.steps ?? [])
        .filter((step) => step.uses?.startsWith('actions/cache/save@'))
        .map((step) => ({ id, step })),
    );
    expect(saves.map(({ id }) => id)).toEqual(['check', 'test', 'package']);
    for (const { step } of saves) {
      expect(step.with?.['path']).toBe('.turbo/cache');
      expect(step.if).toContain("github.ref == 'refs/heads/main'");
    }
  });

  test('no_job_uses_the_combined_cache_action_that_saves_from_pull_requests', () => {
    const combined = YAML_FILES.flatMap(lines).filter((line) =>
      /uses:\s*actions\/cache@/.test(line),
    );
    expect(combined).toEqual([]);
  });

  test('package_uploads_the_release_zips', () => {
    const upload = (ci.jobs?.['package']?.steps ?? []).find((step) =>
      step.uses?.startsWith('actions/upload-artifact@'),
    );
    expect(upload?.with?.['path']).toBe('release/*.zip');
    expect(upload?.with?.['if-no-files-found']).toBe('error');
  });
});

describe('setup-env action', () => {
  const action = parseYaml<Action>(SETUP_ENV);
  const steps = action.runs?.steps ?? [];

  test('is_a_composite_action', () => {
    expect(action.runs?.using).toBe('composite');
  });

  test('installs_bun_from_the_package_json_pin_and_the_toolchain_from_rust_toolchain_toml', () => {
    const bun = steps.find((step) => step.uses?.startsWith('oven-sh/setup-bun@'));
    expect(bun?.with?.['bun-version-file']).toBe('package.json');
    expect(runsOf(steps).some((s) => /rustup toolchain install\s*$/m.test(s))).toBe(true);
  });

  test('restores_the_turbo_cache_but_never_saves_it', () => {
    const restore = steps.find((step) => step.uses?.startsWith('actions/cache/restore@'));
    expect(restore?.with?.['path']).toBe('.turbo/cache');
    expect(steps.some((step) => step.uses?.startsWith('actions/cache/save@'))).toBe(false);
  });

  test('saves_the_cargo_cache_from_main_only', () => {
    const rust = steps.find((step) => step.uses?.startsWith('Swatinem/rust-cache@'));
    expect(String(rust?.with?.['save-if'])).toContain("github.ref == 'refs/heads/main'");
  });

  test('runs_bun_install_with_the_frozen_lockfile', () => {
    expect(runsOf(steps).some(hasInstall)).toBe(true);
  });
});

describe('security workflow', () => {
  const triggers = triggersOf(SECURITY);

  test('cargo_deny_runs_on_a_schedule_and_by_hand_never_per_pull_request', () => {
    expect(triggers).toContain('schedule');
    expect(triggers).toContain('workflow_dispatch');
    expect(triggers).not.toContain('pull_request');
    expect(triggers).not.toContain('push');
  });

  test('the_security_job_runs_cargo_deny', () => {
    const job = parseYaml<Workflow>(SECURITY).jobs?.['security'] ?? {};
    expect(runsOf(expandedSteps(job)).some((s) => /bun run rust:deny/.test(s))).toBe(true);
  });
});

describe('authorship workflow', () => {
  const workflow = parseYaml<Workflow>(AUTHORSHIP);
  const steps = workflow.jobs?.['authorship']?.steps ?? [];

  test('runs_on_pull_requests_including_title_and_body_edits_and_on_pushes_to_main', () => {
    expect(triggersOf(AUTHORSHIP).sort()).toEqual(['pull_request', 'push']);
    const doc = parseYaml<{ on: { pull_request: { types: string[] } } }>(AUTHORSHIP);
    expect(doc.on.pull_request.types).toEqual(
      expect.arrayContaining(['opened', 'edited', 'synchronize', 'reopened']),
    );
  });

  test('checks_out_full_history_for_the_commit_range', () => {
    const checkout = steps.find((step) => step.uses?.startsWith('actions/checkout@'));
    expect(checkout?.with?.['fetch-depth']).toBe(0);
  });

  test('checks_commits_branch_and_pr_text_through_bun_run_attribution', () => {
    const scripts = runsOf(steps).filter((s) => s.includes('bun run attribution'));
    expect(scripts.some((s) => s.includes('--range'))).toBe(true);
    expect(scripts.some((s) => s.includes('--branch'))).toBe(true);
    expect(scripts.some((s) => s.includes('--body-env'))).toBe(true);
  });

  test('pr_title_body_and_branch_reach_the_script_through_env_only', () => {
    const env = steps.flatMap((step) => Object.values(step.env ?? {}).map(String)).join('\n');
    expect(env).toContain('github.event.pull_request.title');
    expect(env).toContain('github.event.pull_request.body');
    expect(env).toContain('github.head_ref');
    for (const script of runsOf(steps)) expect(script).not.toContain('github.');
  });
});

describe('release workflow', () => {
  const release = parseYaml<{ on: { push: { tags: string[]; branches?: string[] } } }>(RELEASE);
  const jobs = parseYaml<Workflow>(RELEASE).jobs ?? {};

  test('runs_only_for_version_tags', () => {
    expect(triggersOf(RELEASE)).toEqual(['push']);
    expect(release.on.push.tags.length).toBeGreaterThan(0);
    expect(release.on.push.branches).toBeUndefined();
  });

  test('reuses_ci_and_publishes_only_after_it_passes', () => {
    expect(jobs['ci']?.uses).toBe('./.github/workflows/ci.yml');
    expect(jobs['publish']?.needs).toBe('ci');
  });

  test('publishes_a_draft_with_the_zips_and_release_notes_from_env_only', () => {
    const steps = jobs['publish']?.steps ?? [];
    expect(steps.some((step) => step.uses?.startsWith('actions/download-artifact@'))).toBe(true);
    const publish = runsOf(steps).find((s) => s.includes('gh release create')) ?? '';
    expect(publish).toContain('release/*.zip');
    expect(publish).toContain('--draft');
    expect(publish).toContain('--verify-tag');
    // Notes come from .changes/releases/<tag>.md when it has content, else from GitHub.
    expect(publish).toContain('--notes-file');
    expect(publish).toContain('--generate-notes');
    expect(publish).toContain('"$TAG"');
  });
});

describe('repository files', () => {
  const REMOVED = [
    '.github/dependabot.yml',
    '.github/mergify.yml',
    '.github/stale.yml',
    '.github/release-drafter.yml',
    '.github/labeler.yml',
    '.github/release.yml',
    '.github/settings.yml',
    '.github/FUNDING.yml',
    `${WORKFLOW_DIR}/cd.yml`,
    `${WORKFLOW_DIR}/verify-changes.yml`,
  ];

  test('unused_automation_files_are_removed', () => {
    expect(REMOVED.filter((path) => existsSync(join(ROOT, path)))).toEqual([]);
  });

  test('legacy_issue_and_pr_templates_are_trimmed', () => {
    expect(filesBelow('.github/ISSUE_TEMPLATE')).toEqual([
      '.github/ISSUE_TEMPLATE/bug_report.yml',
      '.github/ISSUE_TEMPLATE/config.yml',
      '.github/ISSUE_TEMPLATE/feature_request.yml',
    ]);
    expect(filesBelow('.github/PULL_REQUEST_TEMPLATE')).toEqual([]);
    expect(readText('.github/pull_request_template.md')).toContain('bun run check');
  });

  test('issue_forms_are_valid_yaml_forms', () => {
    for (const name of ['bug_report', 'feature_request']) {
      const form = parseYaml<{ name?: string; body?: unknown[] }>(
        `.github/ISSUE_TEMPLATE/${name}.yml`,
      );
      expect(form.name).toBeTruthy();
      expect((form.body ?? []).length).toBeGreaterThan(0);
    }
    const config = parseYaml<{ blank_issues_enabled?: boolean }>(
      '.github/ISSUE_TEMPLATE/config.yml',
    );
    expect(config.blank_issues_enabled).toBe(false);
  });

  test('codeowners_assigns_everything_to_genslate', () => {
    const rules = lines('.github/CODEOWNERS').filter((l) => l.trim() !== '' && !l.startsWith('#'));
    expect(rules).toEqual(['* @GENSLATE']);
  });

  test('security_policy_asks_for_private_reports', () => {
    expect(readText('.github/SECURITY.md')).toContain('security/advisories/new');
  });

  test('no_personal_identifiers_in_github_files', () => {
    const hits = filesBelow('.github')
      .filter((path) => /angeletti/i.test(readText(path)))
      .map((path) => path);
    expect(hits).toEqual([]);
  });

  test('renovate_config_pins_actions_and_uses_commitlint_scopes', () => {
    const renovate = parseJson<Renovate>('.github/renovate.json');
    expect(renovate.$schema).toBe('https://docs.renovatebot.com/renovate-schema.json');
    // config:best-practices pins GitHub Actions to digests, as the workflows are.
    expect(renovate.extends).toContain('config:best-practices');
    expect(renovate.semanticCommits).toBe('enabled');

    const scopes: readonly string[] = commitlint.rules['scope-enum'].flatMap((value) =>
      Array.isArray(value) ? value.map(String) : [],
    );
    const used = [
      renovate.semanticCommitScope,
      ...(renovate.packageRules ?? []).map((r) => r.semanticCommitScope),
    ];
    for (const scope of used) {
      if (scope !== undefined) expect(scopes).toContain(scope);
    }
    expect(renovate.semanticCommitScope).toBe('deps');
  });
});

function parseJson<T>(path: string): T {
  return JSON.parse(readText(path)) as T;
}

/**
 * `bun run setup`: first-time setup. Installs dependencies and the pinned Rust toolchain, installs
 * the cargo tools the checks need and the git hooks, and verifies the machine.
 */
import { defineCommand } from '../lib/args';
import { log } from '../lib/log';
import { fromRoot } from '../lib/paths';
import { capture, run, runOrThrow } from '../lib/run';
import { bunProblem, readBunPin, readRustPin, rustProblem } from '../lib/toolchain';

/** Cargo subcommands `bun run check` needs: `cargo <subcommand>` and the crate that provides it. */
const CARGO_TOOLS = [
  { subcommand: 'deny', crate: 'cargo-deny' },
  { subcommand: 'machete', crate: 'cargo-machete' },
] as const;

const WEBVIEW2_CLIENT = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

await defineCommand({
  name: 'setup',
  summary:
    'Install dependencies, the Rust toolchain, cargo tools and git hooks; check the machine.',
  usage: '[--skip-install]',
  options: {
    'skip-install': { type: 'boolean', description: 'Skip `bun install`.' },
  },
  async run({ values }) {
    const problems: string[] = [];
    const warnings: string[] = [];

    log.title('Bun');
    const bunPin = readBunPin(await Bun.file(fromRoot('package.json')).text());
    const bunIssue = bunProblem(bunPin, Bun.version);
    if (bunIssue === undefined) log.success(`bun ${Bun.version}`);
    else problems.push(bunIssue);

    if (!values['skip-install']) {
      log.title('Dependencies');
      await runOrThrow(['bun', 'install']);
    }

    log.title('Rust');
    const rustPin = readRustPin(await Bun.file(fromRoot('rust-toolchain.toml')).text());
    // Installing once up front stops parallel cargo tasks from racing to auto-install it.
    if ((await run(['rustup', 'toolchain', 'install'])) !== 0) {
      problems.push('rustup toolchain install failed: install rustup (https://rustup.rs)');
    }
    const rustc = await capture(['rustc', '--version']);
    if (rustc.code !== 0) problems.push('rustc not found: install rustup (https://rustup.rs)');
    else {
      const rustIssue = rustProblem(rustPin, rustc.stdout);
      if (rustIssue === undefined) log.success(rustc.stdout.trim());
      else problems.push(rustIssue);
    }

    log.title('Cargo tools');
    for (const tool of CARGO_TOOLS) {
      const found = await capture(['cargo', tool.subcommand, '--version']);
      if (found.code === 0) log.success(found.stdout.trim());
      else if ((await run(['cargo', 'install', '--locked', tool.crate])) !== 0) {
        problems.push(`could not install ${tool.crate}`);
      }
    }

    log.title('Git hooks');
    if ((await run(['bun', 'x', '--no-install', 'lefthook', 'install'])) !== 0) {
      problems.push('lefthook install failed');
    }

    log.title('Windows build prerequisites');
    if (process.platform !== 'win32') {
      warnings.push('SLATECORE builds and runs on Windows only; other systems are unsupported');
    } else {
      const programFiles = process.env['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)';
      const vswhere = `${programFiles}\\Microsoft Visual Studio\\Installer\\vswhere.exe`;
      const msvc = await capture([
        vswhere,
        '-latest',
        '-products',
        '*',
        '-requires',
        'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
        '-property',
        'installationPath',
      ]);
      if (msvc.code === 0 && msvc.stdout.trim() !== '') log.success('MSVC build tools found');
      else {
        warnings.push(
          'MSVC build tools not found: install "Desktop development with C++" (Visual Studio Build Tools)',
        );
      }
      const keys = [
        `HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\${WEBVIEW2_CLIENT}`,
        `HKCU\\Software\\Microsoft\\EdgeUpdate\\Clients\\${WEBVIEW2_CLIENT}`,
      ];
      const found = await Promise.all(
        keys.map((key) => capture(['reg', 'query', key, '/v', 'pv'])),
      );
      if (found.some((result) => result.code === 0)) log.success('WebView2 runtime found');
      else {
        warnings.push(
          'WebView2 runtime not found on this PC (the launcher can use the bundled runtime when packaged)',
        );
      }
    }

    log.title('Summary');
    for (const warning of warnings) log.warn(warning);
    for (const problem of problems) log.error(problem);
    if (problems.length > 0) {
      process.exitCode = 1;
      return;
    }
    log.success('ready: try `bun run dev`');
  },
});

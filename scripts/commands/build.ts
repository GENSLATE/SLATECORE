/** `bun run build [turbo args...]`: production build of every app (`turbo run build`). */
import { runTurbo } from './turbo.util';

process.exit(await runTurbo(['run', 'build', ...Bun.argv.slice(2)]));

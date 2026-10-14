/**
 * The GENSLATE app icons for the browser preview, keyed by app (`explorer`, `terminal`, …).
 * The set ships with the launcher seed (`other/launcher/resources/icons/`). Vite-only
 * (`import.meta.glob`), so the mock backend takes it as an option and tests run without it.
 */
const FILES = import.meta.glob<string>('../../other/launcher/resources/icons/*.svg', {
  query: '?raw',
  import: 'default',
  eager: true,
});

export const MOCK_ICONS: Readonly<Record<string, string>> = Object.fromEntries(
  Object.entries(FILES).map(([path, svg]) => [path.slice(path.lastIndexOf('/') + 1, -4), svg]),
);

/**
 * Which vault files the shell refuses to open: a copy of `src-tauri/src/open_policy.rs` (a
 * test compares the lists). Opening hands the file to Explorer, which for a program, script,
 * shortcut or installer means running it. The shell decides; the UI uses this to skip the
 * "opening leaves traces" note for a file that will be refused anyway, and the browser mock to
 * refuse the same files.
 */

// cspell:ignore accde appinstaller appref appxbundle diagcab jnlp msix msixbundle ppam pyzw
// cspell:ignore searchconnector settingcontent themepack vhdx vsix xbap xlam

/** Extensions (lower case, no dot) of files that run code, install it or point at it. */
export const BLOCKED_EXTENSIONS: ReadonlySet<string> = new Set([
  // Programs and libraries.
  'exe',
  'com',
  'scr',
  'pif',
  'dll',
  'sys',
  'ocx',
  'cpl',
  'msc',
  'jar',
  'gadget',
  // Installers and packages.
  'msi',
  'msp',
  'mst',
  'appx',
  'msix',
  'appxbundle',
  'msixbundle',
  'application',
  'xbap',
  // Scripts and script hosts.
  'bat',
  'cmd',
  'ps1',
  'ps1xml',
  'ps2',
  'ps2xml',
  'psc1',
  'psc2',
  'psd1',
  'psm1',
  'vbs',
  'vbe',
  'js',
  'jse',
  'wsf',
  'wsh',
  'wsc',
  'hta',
  'sct',
  // Shortcuts, links and shell objects.
  'lnk',
  'url',
  'appref-ms',
  'scf',
  'shb',
  'shs',
  'website',
  'settingcontent-ms',
  'library-ms',
  'search-ms',
  'diagcab',
  // Registry and setup information.
  'reg',
  'inf',
  'theme',
  'themepack',
  // Help files that run script, interpreter scripts, update packages and add-ins.
  'chm',
  'py',
  'pyw',
  'pyz',
  'pyzw',
  'msu',
  'cab',
  'appinstaller',
  'jnlp',
  'vsix',
  'searchconnector-ms',
  // Office and Access add-ins and compiled databases (code in the file, no macro prompt).
  'xll',
  'xlam',
  'ppam',
  'mde',
  'accde',
  'ade',
  'adp',
  // Disk images Explorer mounts as a drive.
  'iso',
  'img',
  'vhd',
  'vhdx',
]);

/**
 * Whether the shell refuses to open `path` (a vault path or a file name), read the way Windows
 * reads a name: case-insensitive, trailing dots and spaces dropped, only the last extension
 * counts, and a name that is only an extension (`.exe`) still has it. A stream name
 * (`a.txt:stream`) is refused outright, and so is an extension with a character outside ASCII
 * (a Cyrillic `е` in `exe`, a right-to-left override): no list can vouch for it.
 */
export function isBlockedType(path: string): boolean {
  const name = path.split(/[/\\]/).at(-1) ?? path;
  if (name.includes(':')) return true;
  const trimmed = name.replace(/[. ]+$/, '');
  const dot = trimmed.lastIndexOf('.');
  if (dot === -1) return false;
  const extension = trimmed.slice(dot + 1);
  // biome-ignore lint/suspicious/noControlCharactersInRegex: the ASCII range is the point.
  if (!/^[\x00-\x7f]*$/.test(extension)) return true;
  return BLOCKED_EXTENSIONS.has(extension.toLowerCase());
}

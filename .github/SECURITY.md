# Security policy

SLATECORE LAUNCHER by GENSLATE is a portable Windows app launcher. Its security-sensitive parts
are the encrypted vault and the promise that it writes nothing outside its own folder.

## Supported versions

Only the latest release receives fixes.

## Reporting a vulnerability

Report privately through GitHub:
<https://github.com/GENSLATE/SLATECORE/security/advisories/new>

Please do not open a public issue for a vulnerability. Include the launcher version, the steps to
reproduce, and what an attacker gains. Expect an answer within a week.

## In scope

- Anything that lets someone read vault contents without the password, or that corrupts a vault.
- The launcher writing outside its install folder, or starting something with elevated rights.
- A packaged release that differs from the source it was built from.

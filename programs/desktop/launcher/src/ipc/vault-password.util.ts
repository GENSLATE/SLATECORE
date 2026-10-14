/**
 * The vault's password rule (research/vault.md D5, `genslate-vault`): 8 to 1024 bytes of UTF-8
 * after NFKC normalisation. The UI checks it before sending; the mock enforces it like the crate.
 */
export const VAULT_PASSWORD_MIN_BYTES = 8;
export const VAULT_PASSWORD_MAX_BYTES = 1024;

/** UTF-8 length of the NFKC-normalised password: what the crate measures. */
export function passwordBytes(password: string): number {
  return new TextEncoder().encode(password.normalize('NFKC')).length;
}

export type VaultPasswordProblem = 'too-short' | 'too-long';

export function vaultPasswordProblem(password: string): VaultPasswordProblem | null {
  const bytes = passwordBytes(password);
  if (bytes < VAULT_PASSWORD_MIN_BYTES) return 'too-short';
  if (bytes > VAULT_PASSWORD_MAX_BYTES) return 'too-long';
  return null;
}

import { useToast } from '@genslate/design-system';
import { isCommandError } from '@genslate/tauri-bridge';

import { isVaultError } from '../../ipc/launcher.parse';

/** The message of a failed backend call (command errors, vault errors, plain errors). */
export function errorMessage(error: unknown): string {
  if (isCommandError(error) || isVaultError(error)) return error.message;
  return error instanceof Error ? error.message : String(error);
}

/** Reports a failed backend call as an error toast. */
export function useReport(): (error: unknown) => void {
  const toast = useToast();
  return (error) =>
    toast.add({ title: 'Something went wrong', description: errorMessage(error), type: 'error' });
}

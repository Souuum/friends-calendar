import { writable } from 'svelte/store';
import { api } from '$lib/api';

/**
 * Whether the signed-in user administers the linked Discord server, which is
 * who may change the bot's configuration (`/server`, `/servers`).
 *
 * `null` until the answer arrives, so a page can wait instead of briefly
 * showing "admins only" to an admin. This only decides what the app
 * *shows*: the backend refuses config changes from non-admins on its own,
 * so a stale or tampered value here grants nothing.
 */
export const isAdmin = writable<boolean | null>(null);

let request: Promise<void> | null = null;

/**
 * Asks once per session and shares the answer: every page renders Frame,
 * and each ask costs the backend two Discord calls.
 *
 * Any failure reads as "not an admin" and clears the cached request, so a
 * later page can ask again. Failing open would show config pages whose
 * saves the server is about to refuse.
 */
export function loadAdminStatus(): Promise<void> {
  if (!request) {
    request = (async () => {
      try {
        isAdmin.set(await api.getAdminStatus());
      } catch {
        isAdmin.set(false);
        request = null;
      }
    })();
  }
  return request;
}

/** For tests: forget the cached answer. */
export function resetAdminStatus(): void {
  request = null;
  isAdmin.set(null);
}

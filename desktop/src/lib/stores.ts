import { writable } from 'svelte/store';
import type { User } from './types';

export const user = writable<User | null>(null);
export const isAuthenticated = writable<boolean>(false);
export const isLoading = writable<boolean>(true);
// Shared so the header bell and the sidebar's Notifications badge (both
// live inside Frame.svelte at once) reflect the same number without each
// fetching it independently - see Header.svelte's onMount.
export const unreadNotificationCount = writable<number>(0);

import { writable } from 'svelte/store';
import type { User } from './types';

export const user = writable<User | null>(null);
export const isAuthenticated = writable<boolean>(false);
export const isLoading = writable<boolean>(true);
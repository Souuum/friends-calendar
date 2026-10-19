import { api } from '$lib/api';

type Api = typeof api;
type Overrides = { [K in keyof Api]?: Api[K] };

/**
 * Stubs methods on the shared `api` client for one story, so components that
 * fetch on mount render with fixtures and no backend.
 *
 * Use from a story's `beforeEach`: it returns the cleanup Storybook runs
 * when the story unmounts, which restores the real methods so stubs can't
 * leak into the next story.
 *
 *   beforeEach: () => mockApi({ getFriends: async () => FRIENDS })
 *
 * Methods live on the class prototype, so assigning on the instance shadows
 * them and deleting the own property brings the original back.
 */
export function mockApi(overrides: Overrides): () => void {
  const target = api as unknown as Record<string, unknown>;
  for (const [name, impl] of Object.entries(overrides)) {
    target[name] = impl;
  }
  return () => {
    for (const name of Object.keys(overrides)) {
      delete target[name];
    }
  };
}

/** A promise that never settles, for showing loading states. */
export const pending = <T>() => new Promise<T>(() => {});

/** Resolves after `ms`, so a story can show its in-flight state briefly. */
export const delay = <T>(value: T, ms = 600) =>
  new Promise<T>((resolve) => setTimeout(() => resolve(value), ms));

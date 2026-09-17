import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import Frame from './Frame.svelte';
import { user } from '$lib/stores';
// @ts-expect-error - test-only helper injected by the $app/stores mock below, not a real export
import { __setPathname } from '$app/stores';

const { goto } = vi.hoisted(() => ({ goto: vi.fn() }));
vi.mock('$app/navigation', () => ({ goto }));

// Frame always renders Header (its `{#if user}` checks the stores.ts store
// reference, which is truthy regardless of auth state - a pre-existing
// quirk, not something introduced here), and Header now fetches the
// unread notification count on mount - mock $lib/api so that stays a
// no-op instead of a real network call.
vi.mock('$lib/api', () => ({
  api: { getUnreadNotificationCount: vi.fn().mockResolvedValue(0), clearToken: vi.fn() }
}));

// A settable $page.url.pathname so each test can pick the "current route"
// without needing a real SvelteKit runtime. __setPathname is test-only,
// not a real $app/stores export.
vi.mock('$app/stores', () => {
  let listeners: Array<(value: { url: URL }) => void> = [];
  let current = { url: new URL('http://localhost/') };

  return {
    page: {
      subscribe: (run: (value: { url: URL }) => void) => {
        run(current);
        listeners.push(run);
        return () => {
          listeners = listeners.filter((listener) => listener !== run);
        };
      }
    },
    __setPathname: (pathname: string) => {
      current = { url: new URL(`http://localhost${pathname}`) };
      listeners.forEach((listener) => listener(current));
    }
  };
});

describe('Frame sidebar navigation', () => {
  // Sidebar nav used to be local-only state (`currentView`) that nothing
  // outside Frame could see or change, so clicking a sidebar item did
  // nothing. This locks in the fix: it now actually navigates.
  it('navigates to /announcements when that sidebar item is clicked', async () => {
    __setPathname('/');
    render(Frame);

    await fireEvent.click(within(screen.getByTestId('sidebar')).getByText('Announcements'));

    expect(goto).toHaveBeenCalledWith('/announcements');
  });

  it('highlights the sidebar item matching the current route', () => {
    __setPathname('/announcements');
    render(Frame);

    // Scoped to the rail: the header now names the current screen too, so
    // 'Announcements' appears twice on this page.
    const sidebar = within(screen.getByTestId('sidebar'));
    const announcementButton = sidebar.getByText('Announcements').closest('button');
    const calendarsButton = sidebar.getByText('Calendar').closest('button');

    // aria-current is the real signal now, and unlike a font-weight class it
    // can't be accidentally satisfied by an unrelated utility - the earlier
    // version had to avoid `bg-primary-hover` for exactly that reason.
    expect(announcementButton).toHaveAttribute('aria-current', 'page');
    expect(calendarsButton).not.toHaveAttribute('aria-current');
  });
});

describe('Frame header avatar', () => {
  // This was `let avatarUrl = ...`, evaluated once at init while $user is
  // still null, so it froze at ".../avatars/undefined/undefined.png" for the
  // whole session. Caught by looking at a browser screenshot, not by any
  // assertion - the username beside it rendered correctly, because that
  // reads the store reactively.
  it('updates once the user store is populated', async () => {
    user.set(null);
    render(Frame);

    const before = screen.getByAltText('User avatar') as HTMLImageElement;
    expect(before.src).not.toContain('undefined');

    user.set({
      id: 'u1',
      discord_id: '80351110224678912',
      username: 'someone',
      avatar: 'abc123'
    } as never);
    await tick();

    expect((screen.getByAltText('User avatar') as HTMLImageElement).src).toBe(
      'https://cdn.discordapp.com/avatars/80351110224678912/abc123.png'
    );
  });

  // Accounts that never set an avatar have `avatar: null`, which used to
  // build a URL ending in "null.png" and render as a broken image.
  it('falls back to a Discord default avatar when the user has none', async () => {
    user.set({
      id: 'u1',
      discord_id: '80351110224678912',
      username: 'someone',
      avatar: null
    } as never);
    render(Frame);
    await tick();

    const src = (screen.getByAltText('User avatar') as HTMLImageElement).src;
    expect(src).toMatch(/embed\/avatars\/[0-5]\.png$/);
  });
});

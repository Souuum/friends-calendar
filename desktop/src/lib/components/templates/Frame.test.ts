import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import Frame from './Frame.svelte';
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

    await fireEvent.click(screen.getByText('Announcement'));

    expect(goto).toHaveBeenCalledWith('/announcements');
  });

  it('highlights the sidebar item matching the current route', () => {
    __setPathname('/announcements');
    render(Frame);

    const announcementButton = screen.getByText('Announcement').closest('button');
    const calendarsButton = screen.getByText('Calendars').closest('button');

    // Not `bg-primary-hover` alone - the unconditional `hover:bg-primary-hover`
    // utility class contains that substring regardless of which item is
    // active, so it would pass even when highlighting is broken.
    expect(announcementButton?.className).toContain('font-semibold');
    expect(calendarsButton?.className).not.toContain('font-semibold');
  });
});

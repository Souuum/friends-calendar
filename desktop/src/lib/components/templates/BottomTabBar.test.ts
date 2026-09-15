import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import BottomTabBar from './BottomTabBar.svelte';
// @ts-expect-error - test-only helper injected by the $app/stores mock below, not a real export
import { __setPathname } from '$app/stores';

const { goto } = vi.hoisted(() => ({ goto: vi.fn() }));
vi.mock('$app/navigation', () => ({ goto }));

// Same settable $page.url.pathname pattern as Frame.test.ts.
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

describe('BottomTabBar', () => {
  it('navigates when a tab is clicked', async () => {
    __setPathname('/');
    render(BottomTabBar);

    await fireEvent.click(screen.getByText('Friends'));

    expect(goto).toHaveBeenCalledWith('/friends');
  });

  it('highlights the tab matching the current route', () => {
    __setPathname('/friends');
    render(BottomTabBar);

    const friendsLabel = screen.getByText('Friends');
    const calendarLabel = screen.getByText('Calendar');

    expect(friendsLabel.className).toContain('text-primary');
    expect(calendarLabel.className).not.toContain('text-primary');
  });

  it('maps Hub to /announcements, Alerts to /notifications, and Me to /settings', async () => {
    __setPathname('/');
    render(BottomTabBar);

    await fireEvent.click(screen.getByText('Hub'));
    expect(goto).toHaveBeenLastCalledWith('/announcements');

    await fireEvent.click(screen.getByText('Alerts'));
    expect(goto).toHaveBeenLastCalledWith('/notifications');

    await fireEvent.click(screen.getByText('Me'));
    expect(goto).toHaveBeenLastCalledWith('/settings');
  });

  it('does not show a badge when there are no unread notifications', () => {
    __setPathname('/');
    render(BottomTabBar);

    expect(screen.queryByText('0')).not.toBeInTheDocument();
  });
});

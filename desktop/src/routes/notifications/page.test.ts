import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { unreadNotificationCount } from '$lib/stores';
import type { NotificationInfo } from '$lib/types';

const { getNotifications, markNotificationRead, markAllNotificationsRead, getUnreadNotificationCount } =
  vi.hoisted(() => ({
    getNotifications: vi.fn(),
    markNotificationRead: vi.fn(),
    markAllNotificationsRead: vi.fn(),
    getUnreadNotificationCount: vi.fn()
  }));

vi.mock('$lib/api', () => ({
  api: {
    getNotifications,
    markNotificationRead,
    markAllNotificationsRead,
    getUnreadNotificationCount,
    clearToken: vi.fn()
  }
}));

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

vi.mock('$app/stores', () => ({
  page: {
    subscribe: (run: (value: { url: URL }) => void) => {
      run({ url: new URL('http://localhost/notifications') });
      return () => {};
    }
  }
}));

const { default: NotificationsPage } = await import('./+page.svelte');

function notification(overrides: Partial<NotificationInfo>): NotificationInfo {
  return {
    id: 'n1',
    kind: 'event_invite',
    actor_username: 'alice',
    actor_avatar_url: undefined,
    event_id: undefined,
    message: 'alice invited you to Raclette night',
    read: false,
    created_at: '2026-01-01T12:00:00Z',
    ...overrides
  };
}

describe('notifications page', () => {
  beforeEach(() => {
    getNotifications.mockReset();
    markNotificationRead.mockReset();
    markAllNotificationsRead.mockReset();
    getUnreadNotificationCount.mockReset();
    unreadNotificationCount.set(0);
  });

  it('lists notifications and shows the unread count', async () => {
    getNotifications.mockResolvedValue([
      notification({ id: 'n1', read: false }),
      notification({ id: 'n2', read: true, message: 'bob accepted your event' })
    ]);

    render(NotificationsPage);

    await waitFor(() => expect(screen.getByText('alice invited you to Raclette night')).toBeInTheDocument());
    expect(screen.getByText('bob accepted your event')).toBeInTheDocument();
    expect(screen.getByText('1 unread')).toBeInTheDocument();
  });

  it('shows an empty state when there are no notifications', async () => {
    getNotifications.mockResolvedValue([]);
    render(NotificationsPage);

    await waitFor(() => expect(screen.getByText('Nothing yet.')).toBeInTheDocument());
  });

  it('marks a single notification read on click and refreshes the shared unread count', async () => {
    getNotifications.mockResolvedValue([notification({ id: 'n1', read: false })]);
    markNotificationRead.mockResolvedValue(undefined);
    getUnreadNotificationCount.mockResolvedValue(0);

    render(NotificationsPage);
    await waitFor(() => expect(screen.getByText('1 unread')).toBeInTheDocument());

    await fireEvent.click(screen.getByText('alice invited you to Raclette night'));

    await waitFor(() => expect(markNotificationRead).toHaveBeenCalledWith('n1'));
    await waitFor(() => expect(get(unreadNotificationCount)).toBe(0));
    expect(screen.getByText('0 unread')).toBeInTheDocument();
  });

  it('marks all read', async () => {
    getNotifications.mockResolvedValue([
      notification({ id: 'n1', read: false }),
      notification({ id: 'n2', read: false })
    ]);
    markAllNotificationsRead.mockResolvedValue(undefined);

    render(NotificationsPage);
    await waitFor(() => expect(screen.getByText('2 unread')).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Mark all read' }));

    await waitFor(() => expect(markAllNotificationsRead).toHaveBeenCalledOnce());
    expect(screen.getByText('0 unread')).toBeInTheDocument();
  });

  it('shows an error when loading fails', async () => {
    getNotifications.mockRejectedValue(new Error('network down'));
    render(NotificationsPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('network down'));
  });
});

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte';
import { writable } from 'svelte/store';
import { unreadNotificationCount } from '$lib/stores';

const { getUnreadNotificationCount, goto } = vi.hoisted(() => ({
  getUnreadNotificationCount: vi.fn(),
  goto: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getUnreadNotificationCount, clearToken: vi.fn() }
}));

vi.mock('$app/navigation', () => ({ goto }));

const { default: Header } = await import('./Header.svelte');

// Header receives `user` as the $lib/stores writable itself (Frame.svelte
// passes it straight through), not a plain object - see stores.ts.
const userStore = writable({ username: 'soum', discord_id: 'me-discord', avatar: null });

describe('Header notification bell', () => {
  beforeEach(() => {
    getUnreadNotificationCount.mockReset();
    goto.mockReset();
    unreadNotificationCount.set(0);
  });

  it('shows no unread dot when there are no unread notifications', async () => {
    getUnreadNotificationCount.mockResolvedValue(0);
    render(Header, { user: userStore, avatarUrl: '' });

    await waitFor(() => expect(getUnreadNotificationCount).toHaveBeenCalledOnce());
    expect(screen.queryByTestId('unread-dot')).not.toBeInTheDocument();
  });

  it('shows an unread dot when there are unread notifications', async () => {
    getUnreadNotificationCount.mockResolvedValue(3);
    render(Header, { user: userStore, avatarUrl: '' });

    await waitFor(() => expect(screen.getByTestId('unread-dot')).toBeInTheDocument());
  });

  it('navigates to /notifications when the bell is clicked', async () => {
    getUnreadNotificationCount.mockResolvedValue(0);
    render(Header, { user: userStore, avatarUrl: '' });

    await waitFor(() => expect(getUnreadNotificationCount).toHaveBeenCalledOnce());
    await fireEvent.click(screen.getByLabelText('Notifications'));

    expect(goto).toHaveBeenCalledWith('/notifications');
  });

  it('does not break the header if the unread-count fetch fails', async () => {
    getUnreadNotificationCount.mockRejectedValue(new Error('network down'));
    render(Header, { user: userStore, avatarUrl: '' });

    await waitFor(() => expect(getUnreadNotificationCount).toHaveBeenCalledOnce());
    expect(screen.getByLabelText('Notifications')).toBeInTheDocument();
  });
});

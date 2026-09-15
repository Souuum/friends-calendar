import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { EventWithParticipants, FriendInfo } from '$lib/types';

const { getFriends, getEvents } = vi.hoisted(() => ({
  getFriends: vi.fn(),
  getEvents: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getFriends, getEvents, clearToken: vi.fn(), getToken: vi.fn() }
}));

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

vi.mock('$app/stores', () => ({
  page: {
    subscribe: (run: (value: { url: URL }) => void) => {
      run({ url: new URL('http://localhost/friends') });
      return () => {};
    }
  }
}));

const { default: FriendsPage } = await import('./+page.svelte');

const alice: FriendInfo = {
  user_id: 'alice-id',
  username: 'alice',
  avatar_url: undefined,
  synced_at: '2026-01-01T00:00:00Z'
};
const bob: FriendInfo = {
  user_id: 'bob-id',
  username: 'bob',
  avatar_url: undefined,
  synced_at: '2026-01-01T00:00:00Z'
};

function sharedEvent(): EventWithParticipants {
  return {
    id: 'e1',
    creator_id: 'me-id',
    title: 'Board Game Night',
    start_time: '2099-03-01T19:00:00Z',
    end_time: '2099-03-01T22:00:00Z',
    visibility: 'friends',
    created_at: '2026-02-01T00:00:00Z',
    updated_at: '2026-02-01T00:00:00Z',
    is_creator: true,
    my_status: 'accepted',
    participants: [
      { user_id: 'me-id', username: 'me', status: 'accepted' },
      { user_id: 'alice-id', username: 'alice', status: 'accepted' }
    ]
  };
}

describe('friends directory page', () => {
  beforeEach(() => {
    getFriends.mockReset();
    getEvents.mockReset();
  });

  it('lists synced friends with a shared-event note', async () => {
    getFriends.mockResolvedValue([alice, bob]);
    getEvents.mockResolvedValue([sharedEvent()]);

    render(FriendsPage);

    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());
    expect(screen.getByText('bob')).toBeInTheDocument();
    expect(screen.getByText(/Next: Board Game Night/)).toBeInTheDocument();
    expect(screen.getByText('No shared events')).toBeInTheDocument();
  });

  it('shows an empty state when there are no friends', async () => {
    getFriends.mockResolvedValue([]);
    getEvents.mockResolvedValue([]);

    render(FriendsPage);

    await waitFor(() => expect(screen.getByText('No friends synced yet.')).toBeInTheDocument());
  });

  it('filters by search text', async () => {
    getFriends.mockResolvedValue([alice, bob]);
    getEvents.mockResolvedValue([]);

    render(FriendsPage);
    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());

    await fireEvent.input(screen.getByPlaceholderText('Search by name'), {
      target: { value: 'ali' }
    });

    expect(screen.getByText('alice')).toBeInTheDocument();
    expect(screen.queryByText('bob')).not.toBeInTheDocument();
  });

  it('shows an error when loading fails', async () => {
    getFriends.mockRejectedValue(new Error('network down'));
    getEvents.mockResolvedValue([]);

    render(FriendsPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('network down'));
  });
});

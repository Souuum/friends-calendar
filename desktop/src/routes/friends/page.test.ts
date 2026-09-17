import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import type { EventWithParticipants, FriendInfo, SyncFriendsResult } from '$lib/types';

const { getFriends, getEvents, getFreeFriendsNow, syncFriends, goto, getBestSlots } = vi.hoisted(
  () => ({
    getFriends: vi.fn(),
    getEvents: vi.fn(),
    getFreeFriendsNow: vi.fn(),
    syncFriends: vi.fn(),
    goto: vi.fn(),
    getBestSlots: vi.fn()
  })
);

vi.mock('$lib/api', () => ({
  api: {
    getFriends,
    getEvents,
    getFreeFriendsNow,
    syncFriends,
    clearToken: vi.fn(),
    getToken: vi.fn(),
    getBestSlots
  }
}));

vi.mock('$app/navigation', () => ({ goto }));

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
    reminder_leads: [60],
    is_participant: true,
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
    getFreeFriendsNow.mockReset();
    syncFriends.mockReset();
    goto.mockReset();
    getFreeFriendsNow.mockResolvedValue([]);
    getBestSlots.mockReset();
    getBestSlots.mockResolvedValue([]);
  });

  it('re-syncs friends when "Sync friends" is clicked', async () => {
    getFriends.mockResolvedValue([]);
    getEvents.mockResolvedValue([]);
    const result: SyncFriendsResult = { synced: 1, removed: 0, friends: [alice] };
    syncFriends.mockResolvedValue(result);

    render(FriendsPage);
    await waitFor(() => expect(screen.getByText('No friends synced yet.')).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Sync friends' }));

    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());
    expect(syncFriends).toHaveBeenCalledOnce();
  });

  it('shows a "Free now" pill only for friends the availability endpoint reports free', async () => {
    getFriends.mockResolvedValue([alice, bob]);
    getEvents.mockResolvedValue([]);
    getFreeFriendsNow.mockResolvedValue(['alice-id']);

    render(FriendsPage);
    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());

    const aliceCard = screen.getByText('alice').closest('a');
    const bobCard = screen.getByText('bob').closest('a');
    await waitFor(() => expect(aliceCard).toHaveTextContent('Free now'));
    expect(bobCard).not.toHaveTextContent('Free now');
  });

  it('does not break the page if the availability call fails', async () => {
    getFriends.mockResolvedValue([alice]);
    getEvents.mockResolvedValue([]);
    getFreeFriendsNow.mockRejectedValue(new Error('unavailable'));

    render(FriendsPage);

    await waitFor(() => expect(screen.getByText('alice')).toBeInTheDocument());
    expect(screen.queryByText('Free now')).not.toBeInTheDocument();
  });

  it('navigates to /friends/add when "Add friend" is clicked', async () => {
    getFriends.mockResolvedValue([]);
    getEvents.mockResolvedValue([]);

    render(FriendsPage);
    await waitFor(() => expect(screen.getByText('No friends synced yet.')).toBeInTheDocument());

    await fireEvent.click(screen.getByRole('button', { name: 'Add friend' }));

    expect(goto).toHaveBeenCalledWith('/friends/add');
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

    await fireEvent.input(screen.getByPlaceholderText('Search by name or Discord tag'), {
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
  describe('the filter chips', () => {
    const recently: FriendInfo = {
      user_id: 'new-id',
      username: 'newcomer',
      avatar_url: undefined,
      synced_at: new Date().toISOString()
    };
    const longAgo: FriendInfo = {
      user_id: 'old-id',
      username: 'veteran',
      avatar_url: undefined,
      synced_at: new Date(Date.now() - 60 * 24 * 60 * 60 * 1000).toISOString()
    };

    it('narrows to people free at some point this week', async () => {
      getFriends.mockResolvedValue([recently, longAgo]);
      getEvents.mockResolvedValue([]);
      getFreeFriendsNow.mockResolvedValue([]);
      // One request for the whole week, not one per friend.
      getBestSlots.mockResolvedValue([
        { start: '2027-01-01T19:00:00Z', free_count: 1, free_friend_ids: ['new-id'] }
      ]);

      render(FriendsPage);
      await waitFor(() => expect(screen.getByText('veteran')).toBeInTheDocument());

      await fireEvent.click(screen.getByRole('button', { name: 'Free this week' }));

      expect(screen.getByText('newcomer')).toBeInTheDocument();
      expect(screen.queryByText('veteran')).not.toBeInTheDocument();
    });

    it('narrows to people synced in the last week', async () => {
      getFriends.mockResolvedValue([recently, longAgo]);
      getEvents.mockResolvedValue([]);
      getFreeFriendsNow.mockResolvedValue([]);

      render(FriendsPage);
      await waitFor(() => expect(screen.getByText('veteran')).toBeInTheDocument());

      await fireEvent.click(screen.getByRole('button', { name: 'Recently added' }));

      expect(screen.getByText('newcomer')).toBeInTheDocument();
      expect(screen.queryByText('veteran')).not.toBeInTheDocument();
    });

    it('goes back to everyone', async () => {
      getFriends.mockResolvedValue([recently, longAgo]);
      getEvents.mockResolvedValue([]);
      getFreeFriendsNow.mockResolvedValue([]);

      render(FriendsPage);
      await waitFor(() => expect(screen.getByText('veteran')).toBeInTheDocument());

      await fireEvent.click(screen.getByRole('button', { name: 'Recently added' }));
      await fireEvent.click(screen.getByRole('button', { name: 'All' }));

      expect(screen.getByText('veteran')).toBeInTheDocument();
    });

    // A filtered-to-nothing list should say which filter did it, not
    // "no friends match your search" when you never typed one.
    it('says why the list is empty', async () => {
      getFriends.mockResolvedValue([longAgo]);
      getEvents.mockResolvedValue([]);
      getFreeFriendsNow.mockResolvedValue([]);

      render(FriendsPage);
      await waitFor(() => expect(screen.getByText('veteran')).toBeInTheDocument());

      await fireEvent.click(screen.getByRole('button', { name: 'Recently added' }));

      expect(screen.getByText(/Nobody was added in the last week/)).toBeInTheDocument();
    });

    // The mockup's fourth chip. Pending requests are people who are not
    // friends yet, so it could only ever match nothing.
    it('does not offer a Pending chip that could only be empty', async () => {
      getFriends.mockResolvedValue([recently]);
      getEvents.mockResolvedValue([]);
      getFreeFriendsNow.mockResolvedValue([]);

      render(FriendsPage);
      await waitFor(() => expect(screen.getByText('newcomer')).toBeInTheDocument());

      expect(screen.queryByRole('button', { name: 'Pending' })).not.toBeInTheDocument();
    });
  });
});

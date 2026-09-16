import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import type { DayAvailability, EventWithParticipants, FriendInfo } from '$lib/types';

const { getFriends, getEvents, getWeekAvailability, goto } = vi.hoisted(() => ({
  getFriends: vi.fn(),
  getEvents: vi.fn(),
  getWeekAvailability: vi.fn(),
  goto: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getFriends, getEvents, getWeekAvailability, clearToken: vi.fn(), getToken: vi.fn() }
}));

vi.mock('$app/navigation', () => ({ goto }));

// Both $page.params.id (the route param this page reads) and
// $page.url.pathname (Frame.svelte's nav highlighting) come from the same
// mocked store here.
vi.mock('$app/stores', () => ({
  page: {
    subscribe: (run: (value: { params: { id: string }; url: URL }) => void) => {
      run({ params: { id: 'alice-id' }, url: new URL('http://localhost/friends/alice-id') });
      return () => {};
    }
  }
}));

const { default: FriendDetailPage } = await import('./+page.svelte');

const alice: FriendInfo = {
  user_id: 'alice-id',
  username: 'alice',
  avatar_url: undefined,
  synced_at: '2026-01-01T00:00:00Z'
};

function eventWith(id: string, title: string, participantIds: string[]): EventWithParticipants {
  return {
    id,
    creator_id: 'me-id',
    title,
    start_time: '2026-03-01T19:00:00Z',
    end_time: '2026-03-01T22:00:00Z',
    visibility: 'friends',
    created_at: '2026-02-01T00:00:00Z',
    updated_at: '2026-02-01T00:00:00Z',
    reminder_leads: [60],
    is_participant: true,
    is_creator: true,
    my_status: 'accepted',
    participants: participantIds.map((id) => ({ user_id: id, username: id, status: 'accepted' }))
  };
}

function sevenDays(freeCounts: number[]): DayAvailability[] {
  return freeCounts.map((count, i) => ({
    date: `2026-03-0${i + 1}T00:00:00Z`,
    free_user_ids: Array.from({ length: count }, (_, j) => `user-${j}`)
  }));
}

describe('friend detail page', () => {
  beforeEach(() => {
    getFriends.mockReset();
    getEvents.mockReset();
    getWeekAvailability.mockReset();
    getWeekAvailability.mockResolvedValue(sevenDays([2, 2, 2, 2, 2, 2, 2]));
  });

  it('shows the weekly availability strip', async () => {
    getFriends.mockResolvedValue([alice]);
    getEvents.mockResolvedValue([]);
    getWeekAvailability.mockResolvedValue(sevenDays([2, 1, 0, 2, 1, 0, 2]));

    render(FriendDetailPage);

    await waitFor(() => expect(screen.getByText('Free this week')).toBeInTheDocument());
    expect(getWeekAvailability).toHaveBeenCalledWith('alice-id', expect.any(String));
  });

  it('does not show the strip if the availability call fails', async () => {
    getFriends.mockResolvedValue([alice]);
    getEvents.mockResolvedValue([]);
    getWeekAvailability.mockRejectedValue(new Error('not friends'));

    render(FriendDetailPage);

    await waitFor(() => expect(screen.getByRole('heading', { name: 'alice' })).toBeInTheDocument());
    expect(screen.queryByText('Free this week')).not.toBeInTheDocument();
  });

  it('shows the friend and only events shared with them', async () => {
    getFriends.mockResolvedValue([alice]);
    getEvents.mockResolvedValue([
      eventWith('shared', 'Raclette night', ['me-id', 'alice-id']),
      eventWith('not-shared', 'Solo errand', ['me-id'])
    ]);

    render(FriendDetailPage);

    await waitFor(() => expect(screen.getByRole('heading', { name: 'alice' })).toBeInTheDocument());
    expect(screen.getByText('Raclette night')).toBeInTheDocument();
    expect(screen.queryByText('Solo errand')).not.toBeInTheDocument();
  });

  it('shows a not-found state when the id matches no friend', async () => {
    getFriends.mockResolvedValue([]);
    getEvents.mockResolvedValue([]);

    render(FriendDetailPage);

    await waitFor(() => expect(screen.getByText('Friend not found.')).toBeInTheDocument());
  });

  it('shows an error when loading fails', async () => {
    getFriends.mockRejectedValue(new Error('network down'));
    getEvents.mockResolvedValue([]);

    render(FriendDetailPage);

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('network down'));
  });
});

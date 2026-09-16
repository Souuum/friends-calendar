import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import Calendar from './Calendar.svelte';
import type { EventWithParticipants, FriendInfo } from '$lib/types';

const { getFreeFriendsNow, getFriends } = vi.hoisted(() => ({
  getFreeFriendsNow: vi.fn(),
  getFriends: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getFreeFriendsNow, getFriends }
}));

const alice: FriendInfo = {
  user_id: 'alice-id',
  username: 'alice',
  avatar_url: undefined,
  synced_at: '2026-01-01T00:00:00Z'
};

function today(hour: number): string {
  const d = new Date();
  d.setHours(hour, 0, 0, 0);
  return d.toISOString();
}

function makeEvent(overrides: Partial<EventWithParticipants> = {}): EventWithParticipants {
  return {
    id: 'e1',
    creator_id: 'other-id',
    title: 'Board Game Night',
    start_time: today(19),
    end_time: today(22),
    visibility: 'friends',
    created_at: '2026-02-01T00:00:00Z',
    updated_at: '2026-02-01T00:00:00Z',
    reminder_leads: [60],
    is_participant: true,
    is_creator: false,
    my_status: 'accepted',
    participants: [{ user_id: 'me-id', username: 'me', status: 'accepted' }],
    ...overrides
  };
}

describe('Calendar', () => {
  beforeEach(() => {
    getFreeFriendsNow.mockReset();
    getFriends.mockReset();
  });

  it('shows free-tonight friends when available', async () => {
    getFreeFriendsNow.mockResolvedValue(['alice-id']);
    getFriends.mockResolvedValue([alice]);

    render(Calendar, { props: { events: [] } });

    await waitFor(() => expect(screen.getByText('1 friends have nothing on')).toBeInTheDocument());
  });

  it('shows an empty-state message when nobody is free', async () => {
    getFreeFriendsNow.mockResolvedValue([]);
    getFriends.mockResolvedValue([alice]);

    render(Calendar, { props: { events: [] } });

    await waitFor(() => expect(screen.getByText('No friends free right now')).toBeInTheDocument());
  });

  it('filters events by "Created by me" without a refetch', async () => {
    getFreeFriendsNow.mockResolvedValue([]);
    getFriends.mockResolvedValue([]);

    const mine = makeEvent({ id: 'mine', title: 'My Event', is_creator: true });
    const theirs = makeEvent({ id: 'theirs', title: 'Their Event', is_creator: false });

    render(Calendar, { props: { events: [mine, theirs] } });

    await waitFor(() => expect(screen.getByText('My Event')).toBeInTheDocument());
    expect(screen.getByText('Their Event')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Created by me' }));

    expect(screen.getByText('My Event')).toBeInTheDocument();
    expect(screen.queryByText('Their Event')).not.toBeInTheDocument();
    expect(getFreeFriendsNow).toHaveBeenCalledOnce();
  });

  it('does not count discovered events as awaiting your answer', async () => {
    getFreeFriendsNow.mockResolvedValue([]);
    getFriends.mockResolvedValue([]);

    // Both have no my_status. Only the first is one you owe an answer on -
    // the second is just visible to you (public, or a friend's event).
    const invited = makeEvent({
      id: 'invited',
      title: 'Invited Event',
      is_creator: false,
      reminder_leads: [60],
      is_participant: true,
      my_status: 'pending'
    });
    const discovered = makeEvent({
      id: 'discovered',
      title: 'Discovered Event',
      is_creator: false,
      reminder_leads: [60],
      is_participant: false,
      my_status: undefined
    });

    render(Calendar, { props: { events: [invited, discovered] } });

    await waitFor(() => expect(screen.getByText('Invited Event')).toBeInTheDocument());
    expect(screen.getByText('Discovered Event')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Awaiting my answer' }));

    expect(screen.getByText('Invited Event')).toBeInTheDocument();
    expect(screen.queryByText('Discovered Event')).not.toBeInTheDocument();
  });

  it('selects an event into the peek panel on click', async () => {
    getFreeFriendsNow.mockResolvedValue([]);
    getFriends.mockResolvedValue([]);

    render(Calendar, { props: { events: [makeEvent({ title: 'Board Game Night' })] } });

    expect(screen.getByText('Select an event to see its details here.')).toBeInTheDocument();

    await fireEvent.click(screen.getAllByText('Board Game Night')[0]);

    await waitFor(() => expect(screen.getByText('1 invited')).toBeInTheDocument());
  });

  it('switches to the agenda list, which shows upcoming events grouped by day', async () => {
    getFreeFriendsNow.mockResolvedValue([]);
    getFriends.mockResolvedValue([]);

    const soon = new Date();
    soon.setDate(soon.getDate() + 1);
    soon.setHours(19, 0, 0, 0);

    render(Calendar, {
      props: {
        events: [makeEvent({ id: 'e1', title: 'Board Game Night', start_time: soon.toISOString() })]
      }
    });

    await fireEvent.click(screen.getByRole('button', { name: 'List' }));

    // Day-grouped heading rather than a grid cell.
    expect(screen.getByText('Tomorrow')).toBeInTheDocument();
    expect(screen.getByText('Board Game Night')).toBeInTheDocument();
  });

  it('opens the peek panel from an agenda row, same as from the grid', async () => {
    getFreeFriendsNow.mockResolvedValue([]);
    getFriends.mockResolvedValue([]);

    const soon = new Date();
    soon.setDate(soon.getDate() + 1);
    soon.setHours(19, 0, 0, 0);

    render(Calendar, {
      props: { events: [makeEvent({ title: 'Raclette', start_time: soon.toISOString() })] }
    });

    await fireEvent.click(screen.getByRole('button', { name: 'List' }));
    await fireEvent.click(screen.getByText('Raclette'));

    await waitFor(() => expect(screen.getByText('1 invited')).toBeInTheDocument());
  });
});

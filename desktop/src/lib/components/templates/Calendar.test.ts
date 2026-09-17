import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import Calendar from './Calendar.svelte';
import type { EventWithParticipants, FriendInfo } from '$lib/types';

const { getFreeFriendsNow, getFriends, getServers, previewAnnouncement } = vi.hoisted(() => ({
  getFreeFriendsNow: vi.fn(),
  getFriends: vi.fn(),
  // CreateEventModal loads these on mount. Stubbed here because opening the
  // create form is now reachable from the calendar itself (double-click a
  // day), not only from a test that renders the modal directly.
  getServers: vi.fn(),
  previewAnnouncement: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: { getFreeFriendsNow, getFriends, getServers, previewAnnouncement }
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
    getServers.mockReset();
    previewAnnouncement.mockReset();
    // A default so opening the create form doesn't explode on an unstubbed
    // call; tests that care about specific friends override it.
    getFriends.mockResolvedValue([]);
    getFreeFriendsNow.mockResolvedValue([]);
    getServers.mockResolvedValue({ guilds: [], invite_url: '' });
    previewAnnouncement.mockResolvedValue('');
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
  describe('tapping a day', () => {
    /** The cell for today, which `makeEvent` puts its events on. */
    function todayCell() {
      const label = String(new Date().getDate());
      return screen
        .getAllByRole('button')
        .find(
          (el) => el.getAttribute('aria-pressed') !== null && el.textContent?.startsWith(label)
        );
    }

    it("lists that day's events under the grid", async () => {
      render(Calendar, { props: { events: [makeEvent({ title: 'Board Game Night' })] } });

      await fireEvent.click(todayCell()!);

      const list = await screen.findByRole('region', { hidden: true }).catch(() => null);
      void list;
      expect(screen.getByText(/1 event$/)).toBeInTheDocument();
      // The title appears in the grid chip too, so assert the count line -
      // that only exists in the day list.
    });

    it('clears the selection when the same day is tapped again', async () => {
      render(Calendar, { props: { events: [makeEvent()] } });

      await fireEvent.click(todayCell()!);
      expect(screen.getByText(/1 event$/)).toBeInTheDocument();

      await fireEvent.click(todayCell()!);
      expect(screen.queryByText(/1 event$/)).not.toBeInTheDocument();
    });

    it('says so when the day has nothing on it', async () => {
      render(Calendar, { props: { events: [] } });

      await fireEvent.click(todayCell()!);

      expect(screen.getByText(/Nothing on this day/)).toBeInTheDocument();
    });

    // The chip is inside the cell, so without stopPropagation one tap would
    // both open the event and change which day filters the list.
    it('opening an event does not also select its day', async () => {
      render(Calendar, { props: { events: [makeEvent({ title: 'Board Game Night' })] } });

      const chip = screen.getByText('Board Game Night');
      await fireEvent.click(chip);

      // The peek panel opened...
      expect(screen.getByTestId('event-peek')).toBeInTheDocument();
      // ...and no day list appeared.
      expect(screen.queryByText(/1 event$/)).not.toBeInTheDocument();
    });

    // The cells have claimed role="button" since they were written.
    it('responds to Enter on a focused day', async () => {
      render(Calendar, { props: { events: [makeEvent()] } });

      await fireEvent.keyDown(todayCell()!, { key: 'Enter' });

      expect(screen.getByText(/1 event$/)).toBeInTheDocument();
    });
  });

  describe('starting an event on a day', () => {
    it('double-click opens the create form prefilled with that date', async () => {
      render(Calendar, { props: { events: [] } });

      const label = String(new Date().getDate());
      const cell = screen
        .getAllByRole('button')
        .find(
          (el) => el.getAttribute('aria-pressed') !== null && el.textContent?.startsWith(label)
        );
      await fireEvent.dblClick(cell!);

      // Assert the value in the field, not that a prop was passed.
      // The value in the field, not that a prop was passed.
      const start = (await screen.findByLabelText(/Start Time/i)) as HTMLInputElement;
      const now = new Date();
      const localDay = [
        now.getFullYear(),
        String(now.getMonth() + 1).padStart(2, '0'),
        String(now.getDate()).padStart(2, '0')
      ].join('-');
      expect(start.value).toBe(`${localDay}T19:00`);

      // And an end time, so the form is submittable without inventing one.
      const end = screen.getByLabelText(/End Time/i) as HTMLInputElement;
      expect(end.value).toBe(`${localDay}T21:00`);
    });
  });
});

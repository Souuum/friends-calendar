import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';

const { getEvents, getFriends, getFreeFriendsNow } = vi.hoisted(() => ({
  getEvents: vi.fn(),
  getFriends: vi.fn(),
  getFreeFriendsNow: vi.fn()
}));

vi.mock('$lib/api', () => ({
  api: {
    getEvents,
    getFriends,
    getFreeFriendsNow,
    getServers: vi.fn().mockResolvedValue({ guilds: [], invite_url: '' }),
    getUnreadNotificationCount: vi.fn().mockResolvedValue(0),
    clearToken: vi.fn()
  }
}));

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

const { default: CalendarView } = await import('./CalendarView.svelte');

describe('CalendarView', () => {
  beforeEach(() => {
    getEvents.mockReset();
    getFriends.mockReset().mockResolvedValue([]);
    getFreeFriendsNow.mockReset().mockResolvedValue([]);
  });

  // The regression this file exists for. An empty calendar used to be
  // replaced entirely by a "No events yet" message - but "+ New Event"
  // lives in CalendarHeader, inside Calendar, so a fresh account had no
  // way to create its first event and stayed empty forever. A brand new
  // deployment landed in exactly that state.
  it('still renders the calendar when there are no events', async () => {
    getEvents.mockResolvedValue([]);

    render(CalendarView);

    await waitFor(() => expect(screen.getByText('+ New Event')).toBeInTheDocument());
  });

  it('shows a hint alongside the calendar rather than instead of it', async () => {
    getEvents.mockResolvedValue([]);

    render(CalendarView);

    await waitFor(() => expect(screen.getByText(/No events yet/)).toBeInTheDocument());
    // Both, not either.
    expect(screen.getByText('+ New Event')).toBeInTheDocument();
  });

  it('surfaces a load failure instead of an empty calendar', async () => {
    getEvents.mockRejectedValue(new Error('network down'));

    render(CalendarView);

    await waitFor(() => expect(screen.getByText('network down')).toBeInTheDocument());
  });
});

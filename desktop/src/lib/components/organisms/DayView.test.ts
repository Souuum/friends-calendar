import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import DayView from './DayView.svelte';
import type { EventWithParticipants } from '$lib/types';

const DAY = new Date(2026, 8, 17);

function event(hour: number, title: string): EventWithParticipants {
  const start = new Date(DAY);
  start.setHours(hour, 0, 0, 0);
  return {
    id: title,
    creator_id: 'c',
    title,
    start_time: start.toISOString(),
    end_time: new Date(start.getTime() + 3600_000).toISOString(),
    visibility: 'friends',
    created_at: start.toISOString(),
    updated_at: start.toISOString(),
    participants: [],
    is_creator: false,
    is_participant: true,
    reminder_leads: []
  } as EventWithParticipants;
}

describe('DayView', () => {
  it('renders the hour grid when there are events', () => {
    render(DayView, { props: { events: [event(19, 'Board games')] } });
    expect(screen.getByText('Board games')).toBeInTheDocument();
  });

  // This used to render *after* the grid, so an empty day meant scrolling
  // past 24 hours of nothing to reach the message saying there was nothing.
  it('replaces the grid with the empty state rather than appending to it', () => {
    render(DayView, { props: { events: [] } });

    expect(screen.getByText('No events today')).toBeInTheDocument();
    // No hour labels, because the grid isn't rendered at all.
    expect(screen.queryByText('12 AM')).not.toBeInTheDocument();
    expect(screen.queryByText('12:00 AM')).not.toBeInTheDocument();
  });

  // Calendar.svelte's headerDate already renders exactly this string above
  // the component when view === 'day'. Rendering it again here showed the
  // date twice, which costs a line of vertical space on a phone.
  it('leaves the date heading to the calendar header', () => {
    render(DayView, { props: { events: [] } });
    expect(screen.queryByText(/September 17, 2026/)).not.toBeInTheDocument();
  });
});

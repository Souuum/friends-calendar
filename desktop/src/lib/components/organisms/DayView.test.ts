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

/**
 * Imported busy blocks.
 *
 * ⚠️ These are context, not events. The guarantees worth holding are that
 * they show up, that they carry no title (there is none to carry), and that
 * they are not interactive - a block with nothing behind it must not look
 * like something you can open.
 */
describe('DayView busy blocks', () => {
  function busy(startHour: number, endHour: number) {
    const start = new Date(DAY);
    start.setHours(startHour, 0, 0, 0);
    const end = new Date(DAY);
    end.setHours(endHour, 0, 0, 0);
    return { starts_at: start.toISOString(), ends_at: end.toISOString() };
  }

  it('draws a band for an imported block', () => {
    render(DayView, { props: { events: [], busy: [busy(9, 10)], day: DAY } });

    expect(screen.getAllByTestId('busy-band')).toHaveLength(1);
    expect(screen.getByText('Busy')).toBeInTheDocument();
  });

  // A day with a work calendar full of meetings and no app events is not an
  // empty day, and saying so would contradict the band right next to it.
  it('does not claim the day is empty when only busy blocks are on it', () => {
    render(DayView, { props: { events: [], busy: [busy(9, 10)], day: DAY } });

    expect(screen.queryByText('No events today')).not.toBeInTheDocument();
  });

  it('still shows the empty state when there is genuinely nothing', () => {
    render(DayView, { props: { events: [], busy: [], day: DAY } });

    expect(screen.getByText('No events today')).toBeInTheDocument();
  });

  // ⚠️ Not a button and not clickable: there is no detail view for a block
  // that has no title, no participants and no RSVP, so looking clickable
  // would promise something that cannot exist.
  it('renders busy blocks as non-interactive', () => {
    render(DayView, { props: { events: [], busy: [busy(9, 10)], day: DAY } });

    const band = screen.getAllByTestId('busy-band')[0];
    expect(band.tagName).not.toBe('BUTTON');
    expect(band.closest('button')).toBeNull();
    expect(band.className).toContain('pointer-events-none');
  });

  it('ignores blocks belonging to another day', () => {
    const other = new Date(DAY);
    other.setDate(other.getDate() + 3);
    const start = new Date(other);
    start.setHours(9, 0, 0, 0);
    const end = new Date(other);
    end.setHours(10, 0, 0, 0);

    render(DayView, {
      props: {
        events: [],
        busy: [{ starts_at: start.toISOString(), ends_at: end.toISOString() }],
        day: DAY
      }
    });

    expect(screen.queryAllByTestId('busy-band')).toHaveLength(0);
  });
});

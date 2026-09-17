import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import WeekView from './WeekView.svelte';
import type { EventWithParticipants } from '$lib/types';

const MONDAY = new Date(2026, 8, 14);

function week(start: Date): Date[] {
  return Array.from({ length: 7 }, (_, i) => {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    return d;
  });
}

function event(day: Date, hour: number, title: string): EventWithParticipants {
  const start = new Date(day);
  start.setHours(hour, 0, 0, 0);
  return {
    id: `${title}-${day.toDateString()}`,
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

describe('WeekView mobile day strip', () => {
  beforeEach(() => vi.useRealTimers());

  it('offers every day of the week as a control', () => {
    const days = week(MONDAY);
    render(WeekView, { props: { weekDays: days, eventsForDay: () => [] } });

    const strip = screen.getByTestId('week-view-mobile');
    // 7 day buttons, whatever else the desktop half renders.
    const buttons = strip.querySelectorAll('button[aria-pressed]');
    expect(buttons).toHaveLength(7);
  });

  // The narrow layout shows one day at a time; without this it would open
  // on Monday regardless of what day it actually is.
  it('opens on today when today is inside the week', () => {
    vi.useFakeTimers();
    const days = week(MONDAY);
    vi.setSystemTime(days[3]); // Thursday

    render(WeekView, { props: { weekDays: days, eventsForDay: () => [] } });

    const pressed = screen
      .getByTestId('week-view-mobile')
      .querySelector('button[aria-pressed="true"]');
    expect(pressed?.getAttribute('aria-label')).toContain('Thursday');
  });

  it('falls back to the first day for a week that does not contain today', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 0, 1));

    render(WeekView, { props: { weekDays: week(MONDAY), eventsForDay: () => [] } });

    const pressed = screen
      .getByTestId('week-view-mobile')
      .querySelector('button[aria-pressed="true"]');
    expect(pressed?.getAttribute('aria-label')).toContain('Monday');
  });

  it('shows the tapped day rather than the default one', async () => {
    const days = week(MONDAY);
    const wednesday = event(days[2], 19, 'Board games');
    render(WeekView, {
      props: {
        weekDays: days,
        eventsForDay: (d: Date) => (d.getDate() === days[2].getDate() ? [wednesday] : [])
      }
    });

    const strip = screen.getByTestId('week-view-mobile');
    expect(strip.textContent).toContain('Nothing on this day');

    const wed = Array.from(strip.querySelectorAll('button[aria-pressed]')).find((b) =>
      b.getAttribute('aria-label')?.includes('Wednesday')
    )!;
    await fireEvent.click(wed);

    expect(strip.textContent).toContain('Board games');
  });

  it('says so when the selected day is empty, instead of a bare grid', () => {
    render(WeekView, { props: { weekDays: week(MONDAY), eventsForDay: () => [] } });
    expect(screen.getByTestId('week-view-mobile').textContent).toContain('Nothing on this day');
  });

  // Both layouts are mounted and toggled with `hidden`/`md:` classes, so
  // the wide grid must keep rendering every day.
  it('still renders all seven columns for the wide layout', () => {
    const days = week(MONDAY);
    render(WeekView, { props: { weekDays: days, eventsForDay: () => [] } });

    const desktop = screen.getByTestId('week-view-desktop');
    for (const label of ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']) {
      expect(desktop.textContent).toContain(label);
    }
  });
});

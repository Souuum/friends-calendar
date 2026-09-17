import { describe, expect, it } from 'vitest';
import {
  DEFAULT_START_HOUR,
  HOUR_HEIGHT,
  anchorToFirstEvent,
  openingHour,
  openingScrollTop
} from './timeGrid';
import type { EventWithParticipants } from '$lib/types';

function at(hour: number): EventWithParticipants {
  const start = new Date(2026, 8, 17, hour, 0, 0);
  return {
    id: `e-${hour}`,
    creator_id: 'c',
    title: `Event at ${hour}`,
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

describe('openingHour', () => {
  it('opens an hour before the first event, so it is not flush against the top', () => {
    expect(openingHour([at(14), at(9), at(20)])).toBe(8);
  });

  it('falls back to the morning on an empty day rather than midnight', () => {
    expect(openingHour([])).toBe(DEFAULT_START_HOUR);
  });

  // The lead-in must not push the offset negative for an early event.
  it('clamps at midnight for an event in the first hour', () => {
    expect(openingHour([at(0)])).toBe(0);
  });

  it('converts to a pixel offset using the shared row height', () => {
    expect(openingScrollTop([at(10)])).toBe(9 * HOUR_HEIGHT);
  });
});

describe('anchorToFirstEvent', () => {
  it('scrolls the node on mount and again on update', () => {
    const node = { scrollTop: 0 } as HTMLElement;

    const action = anchorToFirstEvent(node, [at(10)]);
    expect(node.scrollTop).toBe(9 * HOUR_HEIGHT);

    // Changing day re-anchors instead of leaving the previous position.
    action.update([at(19)]);
    expect(node.scrollTop).toBe(18 * HOUR_HEIGHT);
  });

  it('anchors an empty day to the morning', () => {
    const node = { scrollTop: 999 } as HTMLElement;
    anchorToFirstEvent(node, []);
    expect(node.scrollTop).toBe(DEFAULT_START_HOUR * HOUR_HEIGHT);
  });
});

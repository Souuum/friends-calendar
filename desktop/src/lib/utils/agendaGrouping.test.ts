import { describe, it, expect } from 'vitest';
import { groupEventsByDay } from './dateUtils';
import type { EventWithParticipants } from '$lib/types';

function evt(id: string, start: Date, title = id): EventWithParticipants {
  return {
    id,
    creator_id: 'c',
    title,
    start_time: start.toISOString(),
    end_time: new Date(start.getTime() + 3600_000).toISOString(),
    visibility: 'friends',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    reminder_lead_minutes: 60,
    is_participant: true,
    is_creator: false,
    my_status: 'accepted',
    participants: []
  };
}

// Built from local parts so the local-day grouping holds in any timezone.
function at(dayOffset: number, hour: number): Date {
  const d = new Date();
  d.setDate(d.getDate() + dayOffset);
  d.setHours(hour, 0, 0, 0);
  return d;
}

describe('groupEventsByDay', () => {
  it('labels today and tomorrow by name, later days by date', () => {
    const now = at(0, 8);
    const groups = groupEventsByDay(
      [evt('a', at(0, 19)), evt('b', at(1, 19)), evt('c', at(5, 19))],
      now
    );

    expect(groups.map((g) => g.label).slice(0, 2)).toEqual(['Today', 'Tomorrow']);
    expect(groups[2].label).not.toBe('Today');
    expect(groups[2].label).not.toBe('Tomorrow');
  });

  it('drops events that have already started', () => {
    const now = at(0, 12);
    const groups = groupEventsByDay([evt('past', at(0, 9)), evt('later', at(0, 18))], now);

    expect(groups.flatMap((g) => g.events.map((e) => e.id))).toEqual(['later']);
  });

  it('sorts chronologically regardless of input order', () => {
    const now = at(0, 8);
    const groups = groupEventsByDay(
      [evt('third', at(2, 10)), evt('first', at(0, 10)), evt('second', at(1, 10))],
      now
    );

    expect(groups.flatMap((g) => g.events.map((e) => e.id))).toEqual([
      'first',
      'second',
      'third'
    ]);
  });

  it('groups several events on one day together', () => {
    const now = at(0, 8);
    const groups = groupEventsByDay([evt('a', at(3, 10)), evt('b', at(3, 20))], now);

    expect(groups).toHaveLength(1);
    expect(groups[0].events.map((e) => e.id)).toEqual(['a', 'b']);
  });

  it('returns nothing when everything is in the past', () => {
    expect(groupEventsByDay([evt('old', at(-2, 10))], at(0, 8))).toEqual([]);
  });
});

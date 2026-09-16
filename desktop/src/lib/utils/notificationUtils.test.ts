import { describe, it, expect } from 'vitest';
import { groupByRecency } from './notificationUtils';
import type { NotificationInfo } from '$lib/types';

function n(id: string, created_at: string): NotificationInfo {
  return {
    id,
    kind: 'event_invite',
    actor_username: 'alice',
    actor_avatar_url: undefined,
    event_id: 'e1',
    message: 'msg',
    read: false,
    created_at
  };
}

// Built from local parts, so these hold in whatever timezone the runner is
// in - a hardcoded UTC string would land on a different calendar day
// depending on the machine.
function localIso(dayOffset: number, hour: number): string {
  const d = new Date();
  d.setDate(d.getDate() + dayOffset);
  d.setHours(hour, 0, 0, 0);
  return d.toISOString();
}

describe('groupByRecency', () => {
  it('splits today from earlier', () => {
    const groups = groupByRecency([n('a', localIso(0, 9)), n('b', localIso(-3, 9))]);

    expect(groups.map((g) => g.label)).toEqual(['Today', 'Earlier']);
    expect(groups[0].notifications.map((x) => x.id)).toEqual(['a']);
    expect(groups[1].notifications.map((x) => x.id)).toEqual(['b']);
  });

  it('treats late yesterday as Earlier, not "within 24 hours"', () => {
    // 23:00 yesterday is under a day old but is still yesterday to a reader.
    const groups = groupByRecency([n('late', localIso(-1, 23))]);
    expect(groups.map((g) => g.label)).toEqual(['Earlier']);
  });

  it('omits empty groups rather than rendering a bare heading', () => {
    expect(groupByRecency([n('a', localIso(0, 9))]).map((g) => g.label)).toEqual(['Today']);
    expect(groupByRecency([n('b', localIso(-5, 9))]).map((g) => g.label)).toEqual(['Earlier']);
    expect(groupByRecency([])).toEqual([]);
  });

  it('preserves the API ordering within a group', () => {
    const groups = groupByRecency([
      n('newest', localIso(0, 18)),
      n('older', localIso(0, 9)),
      n('oldest', localIso(0, 1))
    ]);

    expect(groups[0].notifications.map((x) => x.id)).toEqual(['newest', 'older', 'oldest']);
  });
});

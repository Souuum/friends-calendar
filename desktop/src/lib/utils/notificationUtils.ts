import type { NotificationInfo } from '$lib/types';

export interface NotificationGroup {
  label: 'Today' | 'Earlier';
  notifications: NotificationInfo[];
}

/**
 * Splits notifications into "Today" and "Earlier" for the grouped list.
 *
 * Presentational only - `GET /api/notifications` already returns
 * newest-first, and this preserves that order within each group rather than
 * re-sorting.
 *
 * "Today" means the same *local* calendar day as `now`, not "within the last
 * 24 hours": something from 23:00 yesterday is yesterday to a reader, even
 * though it's under a day old.
 *
 * Empty groups are omitted so the page never renders a heading with nothing
 * under it.
 */
export function groupByRecency(
  notifications: NotificationInfo[],
  now: Date = new Date()
): NotificationGroup[] {
  const isToday = (iso: string) => {
    const d = new Date(iso);
    return (
      d.getFullYear() === now.getFullYear() &&
      d.getMonth() === now.getMonth() &&
      d.getDate() === now.getDate()
    );
  };

  const today = notifications.filter((n) => isToday(n.created_at));
  const earlier = notifications.filter((n) => !isToday(n.created_at));

  const groups: NotificationGroup[] = [];
  if (today.length > 0) groups.push({ label: 'Today', notifications: today });
  if (earlier.length > 0) groups.push({ label: 'Earlier', notifications: earlier });
  return groups;
}

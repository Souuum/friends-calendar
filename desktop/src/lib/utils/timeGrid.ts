import type { EventWithParticipants } from '$lib/types';

/**
 * Height of one hour row, in pixels.
 *
 * This has to agree with `TimeSlot.svelte`'s `h-20` (Tailwind's 5rem) or
 * events drift away from their hour the further down the grid you look.
 * It was previously a bare `80` written twice inside `TimedEvent`, with
 * nothing tying it to the slot height.
 */
export const HOUR_HEIGHT = 80;

/** Hour to open a day grid at when the day has nothing in it. */
export const DEFAULT_START_HOUR = 8;

/**
 * The hour a day grid should be scrolled to on open.
 *
 * Both grids render all 24 hours, so without this they open at midnight
 * and you scroll past the small hours every time to reach anything real.
 * One hour of lead-in keeps the first event off the very top edge, where
 * it reads as cut off.
 */
export function openingHour(events: EventWithParticipants[]): number {
  const hours = events
    .map((event) => new Date(event.start_time).getHours())
    .filter((hour) => Number.isFinite(hour));

  if (hours.length === 0) return DEFAULT_START_HOUR;
  return Math.max(0, Math.min(...hours) - 1);
}

/** Scroll offset in pixels for `openingHour`. */
export function openingScrollTop(events: EventWithParticipants[]): number {
  return openingHour(events) * HOUR_HEIGHT;
}

/**
 * Svelte action: scrolls the element so the day's first event is in view.
 *
 * Re-runs on update, so changing day or week re-anchors rather than
 * leaving you wherever the previous day happened to be scrolled to.
 */
export function anchorToFirstEvent(node: HTMLElement, events: EventWithParticipants[]) {
  const apply = (list: EventWithParticipants[]) => {
    node.scrollTop = openingScrollTop(list);
  };
  apply(events);
  return {
    update: apply
  };
}

import type { ExternalBusy } from '$lib/types';

/**
 * Imported busy blocks, arranged for the calendar grids.
 *
 * ⚠️ These are *not* events and must never be treated as some. They carry no
 * title (the server never reads `SUMMARY`), nothing can be RSVP'd to them,
 * and they belong to no one but the viewer. Keeping them in their own type
 * and their own helpers is what stops them drifting into the event paths -
 * the filter chips, the peek panel and `eventsForDay` all deal in
 * `EventWithParticipants` and none of them should ever see one of these.
 */

/** A block clipped to one calendar day, in local time. */
export interface DayBusy {
  start: Date;
  end: Date;
  /** True when the underlying block runs past this day's edges. */
  clippedStart: boolean;
  clippedEnd: boolean;
}

function startOfDay(day: Date): Date {
  const d = new Date(day);
  d.setHours(0, 0, 0, 0);
  return d;
}

function endOfDay(day: Date): Date {
  const d = startOfDay(day);
  d.setDate(d.getDate() + 1);
  return d;
}

/**
 * The blocks overlapping one local day, clipped to it.
 *
 * ⚠️ Clipping matters: an overnight block (a flight, an on-call shift) would
 * otherwise be positioned from its real start, which on the following day is
 * a negative offset - the band would render above the grid or not at all.
 * Half-open at both ends, so a block ending exactly at midnight belongs to
 * the day it ran in, not the next one.
 */
export function busyForDay(blocks: ExternalBusy[], day: Date): DayBusy[] {
  const from = startOfDay(day);
  const to = endOfDay(day);

  return blocks
    .map((block) => ({ start: new Date(block.starts_at), end: new Date(block.ends_at) }))
    .filter((block) => block.start < to && block.end > from)
    .map((block) => ({
      start: block.start < from ? from : block.start,
      end: block.end > to ? to : block.end,
      clippedStart: block.start < from,
      clippedEnd: block.end > to
    }))
    .sort((a, b) => a.start.getTime() - b.start.getTime());
}

/**
 * Merges blocks that touch or overlap.
 *
 * Two calendars, or a meeting inside a longer "focus time", otherwise draw
 * as stacked bands that look like separate commitments. What the viewer
 * needs from this feature is *when am I not free*, which is the union.
 */
export function mergeBusy(blocks: DayBusy[]): DayBusy[] {
  const merged: DayBusy[] = [];

  for (const block of blocks) {
    const last = merged[merged.length - 1];
    if (last && block.start <= last.end) {
      if (block.end > last.end) {
        last.end = block.end;
        last.clippedEnd = block.clippedEnd;
      }
      continue;
    }
    merged.push({ ...block });
  }

  return merged;
}

/** Both steps, which is what every caller actually wants. */
export function mergedBusyForDay(blocks: ExternalBusy[], day: Date): DayBusy[] {
  return mergeBusy(busyForDay(blocks, day));
}

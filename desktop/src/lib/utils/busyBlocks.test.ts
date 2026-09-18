import { describe, it, expect } from 'vitest';
import { busyForDay, mergeBusy, mergedBusyForDay } from './busyBlocks';
import type { ExternalBusy } from '$lib/types';

/** Local-time ISO, so these tests read the way the grid renders. */
function block(startIso: string, endIso: string): ExternalBusy {
  return { starts_at: new Date(startIso).toISOString(), ends_at: new Date(endIso).toISOString() };
}

const DAY = new Date('2026-03-04T12:00:00');

describe('busyForDay', () => {
  it('keeps a block that falls inside the day', () => {
    const out = busyForDay([block('2026-03-04T09:00', '2026-03-04T10:00')], DAY);

    expect(out).toHaveLength(1);
    expect(out[0].start.getHours()).toBe(9);
    expect(out[0].end.getHours()).toBe(10);
    expect(out[0].clippedStart).toBe(false);
    expect(out[0].clippedEnd).toBe(false);
  });

  it('drops blocks on other days', () => {
    expect(busyForDay([block('2026-03-05T09:00', '2026-03-05T10:00')], DAY)).toHaveLength(0);
  });

  // ⚠️ The case that would render off the grid: an overnight block's real
  // start is *before* this day, so positioning from it gives a negative
  // offset. It has to be clipped to the day being drawn.
  it('clips a block that started the day before', () => {
    const out = busyForDay([block('2026-03-03T22:00', '2026-03-04T06:00')], DAY);

    expect(out).toHaveLength(1);
    expect(out[0].start.getHours()).toBe(0);
    expect(out[0].start.getMinutes()).toBe(0);
    expect(out[0].end.getHours()).toBe(6);
    expect(out[0].clippedStart).toBe(true);
  });

  it('clips a block that runs into the next day', () => {
    const out = busyForDay([block('2026-03-04T22:00', '2026-03-05T06:00')], DAY);

    expect(out[0].start.getHours()).toBe(22);
    expect(out[0].clippedEnd).toBe(true);
    // Midnight *of the next day* is the clip point, not 23:59.
    expect(out[0].end.getDate()).toBe(5);
    expect(out[0].end.getHours()).toBe(0);
  });

  // Half-open: a block ending exactly at midnight ran in the day before it,
  // and must not add a phantom band to a day it never touched.
  it('does not carry a block ending exactly at midnight into the next day', () => {
    expect(busyForDay([block('2026-03-03T22:00', '2026-03-04T00:00')], DAY)).toHaveLength(0);
  });

  it('sorts chronologically', () => {
    const out = busyForDay(
      [
        block('2026-03-04T15:00', '2026-03-04T16:00'),
        block('2026-03-04T09:00', '2026-03-04T10:00')
      ],
      DAY
    );

    expect(out.map((b) => b.start.getHours())).toEqual([9, 15]);
  });
});

describe('mergeBusy', () => {
  // Two calendars, or a meeting inside a longer focus block, otherwise draw
  // as stacked bands that read as separate commitments.
  it('merges overlapping blocks into one', () => {
    const out = mergedBusyForDay(
      [
        block('2026-03-04T09:00', '2026-03-04T11:00'),
        block('2026-03-04T10:00', '2026-03-04T12:00')
      ],
      DAY
    );

    expect(out).toHaveLength(1);
    expect(out[0].start.getHours()).toBe(9);
    expect(out[0].end.getHours()).toBe(12);
  });

  it('merges blocks that merely touch', () => {
    const out = mergedBusyForDay(
      [
        block('2026-03-04T09:00', '2026-03-04T10:00'),
        block('2026-03-04T10:00', '2026-03-04T11:00')
      ],
      DAY
    );

    expect(out).toHaveLength(1);
    expect(out[0].end.getHours()).toBe(11);
  });

  it('keeps a block fully inside another from extending it', () => {
    const out = mergedBusyForDay(
      [
        block('2026-03-04T09:00', '2026-03-04T17:00'),
        block('2026-03-04T10:00', '2026-03-04T11:00')
      ],
      DAY
    );

    expect(out).toHaveLength(1);
    expect(out[0].end.getHours()).toBe(17);
  });

  it('leaves a real gap alone', () => {
    const out = mergedBusyForDay(
      [
        block('2026-03-04T09:00', '2026-03-04T10:00'),
        block('2026-03-04T14:00', '2026-03-04T15:00')
      ],
      DAY
    );

    expect(out).toHaveLength(2);
  });

  it('does not mutate what it was given', () => {
    const first = busyForDay([block('2026-03-04T09:00', '2026-03-04T11:00')], DAY);
    const snapshot = first[0].end.getTime();
    mergeBusy([...first, ...busyForDay([block('2026-03-04T10:00', '2026-03-04T12:00')], DAY)]);

    expect(first[0].end.getTime()).toBe(snapshot);
  });
});

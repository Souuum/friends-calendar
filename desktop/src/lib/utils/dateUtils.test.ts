import { describe, it, expect } from 'vitest';
import { dateUtils } from './dateUtils';

describe('toDatetimeLocalValue', () => {
  it('formats an API timestamp as the local YYYY-MM-DDTHH:mm an input accepts', () => {
    // Built from local parts so the assertion holds in any timezone the
    // test runner happens to be in - hardcoding a UTC string and an
    // expected local output would only pass on one machine.
    const local = new Date(2026, 8, 20, 18, 5);
    expect(dateUtils.toDatetimeLocalValue(local.toISOString())).toBe('2026-09-20T18:05');
  });

  it('pads single-digit months, days, hours and minutes', () => {
    const local = new Date(2026, 0, 2, 3, 4);
    expect(dateUtils.toDatetimeLocalValue(local.toISOString())).toBe('2026-01-02T03:04');
  });

  it('returns an empty string for an unparseable value instead of "Invalid Date"', () => {
    expect(dateUtils.toDatetimeLocalValue('not a date')).toBe('');
  });

  it('does not emit seconds or a zone suffix', () => {
    const value = dateUtils.toDatetimeLocalValue(new Date(2026, 8, 20, 18, 5, 30).toISOString());
    expect(value).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/);
  });
});

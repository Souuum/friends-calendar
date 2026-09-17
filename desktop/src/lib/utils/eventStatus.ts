/**
 * The mockup's own STATUS map, in one place.
 *
 * It was previously inlined in `EventPeekPanel` and needed again by the
 * month-grid chip, which is exactly how two components end up disagreeing
 * about what "maybe" looks like. Values are lifted verbatim from
 * `Friends Calendar Mockups.dc.html`'s `STATUS` object - "accepted" is
 * tint/accent, never green.
 */
export type StatusKey = 'accepted' | 'maybe' | 'declined' | 'pending';

export interface StatusStyle {
  label: string;
  /** The 3px rail on a chip, and the bar across the top of the peek panel. */
  bar: string;
  /** Pill background. */
  bg: string;
  /** Pill text. */
  fg: string;
}

export const STATUS: Record<StatusKey, StatusStyle> = {
  accepted: { label: 'Going', bar: '#5030e5', bg: '#eee8ff', fg: '#5030e5' },
  maybe: { label: 'Maybe', bar: '#fbb13c', bg: 'rgba(251,177,60,0.18)', fg: '#a9700f' },
  declined: { label: "Can't", bar: '#fb2c2c', bg: 'rgba(251,44,44,0.14)', fg: '#c01a1a' },
  pending: { label: 'No answer', bar: '#7c7c83', bg: '#f4f4f7', fg: '#5c5c61' }
};

export function statusOf(status?: string): StatusStyle {
  return STATUS[(status ?? 'pending') as StatusKey] ?? STATUS.pending;
}

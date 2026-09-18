<script lang="ts">
  import type { DayBusy } from '$lib/utils/busyBlocks';
  import { HOUR_HEIGHT } from '$lib/utils/timeGrid';

  /** The block, already clipped to the day this grid is showing. */
  export let block: DayBusy;
  /** The hour row this band is being drawn inside. */
  export let hour: number;

  // Same coordinate system as TimedEvent: offset within the hour row, and a
  // height in hours. HOUR_HEIGHT rather than a literal, or bands drift away
  // from the events beside them further down the grid.
  $: top = ((block.start.getHours() === hour ? block.start.getMinutes() : 0) / 60) * HOUR_HEIGHT;
  $: height = Math.max(
    ((block.end.getTime() - block.start.getTime()) / 3_600_000) * HOUR_HEIGHT,
    12
  );

  function time(date: Date): string {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  $: label = `Busy ${block.clippedStart ? 'until' : 'from'} ${time(block.clippedStart ? block.end : block.start)}`;
</script>

<!--
  ⚠️ A div, not a button, and `pointer-events-none`. There is nothing to open:
  the block has no title, no participants and no RSVP, and making it look
  clickable would promise a detail view that cannot exist. It also must not
  swallow clicks meant for the day cell underneath.

  Striped rather than filled so it reads as "unavailable" instead of as
  another event, and sits behind real events (`z-0` against their stacking)
  because an event you can act on matters more than context.
-->
<div
  class="pointer-events-none absolute left-1 right-1 z-0 overflow-hidden rounded-md border border-dashed border-line px-2 py-0.5"
  style="top: {top}px; height: {height}px;
         background-image: repeating-linear-gradient(
           45deg,
           var(--color-subtle) 0px,
           var(--color-subtle) 6px,
           transparent 6px,
           transparent 12px
         );"
  aria-label={label}
  data-testid="busy-band"
>
  <span class="text-[10px] font-medium text-muted">Busy</span>
</div>

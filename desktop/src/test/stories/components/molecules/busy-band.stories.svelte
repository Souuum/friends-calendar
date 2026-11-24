<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import type { ComponentProps } from 'svelte';
  import BusyBand from '$lib/components/molecules/BusyBand.svelte';

  const day = (h: number, m = 0) => {
    const d = new Date();
    d.setHours(h, m, 0, 0);
    return d;
  };

  const block = (start: Date, end: Date, clippedStart = false, clippedEnd = false) => ({
    start,
    end,
    clippedStart,
    clippedEnd
  });

  /**
   * A striped, anonymous "Busy" band for time taken by an imported calendar.
   * Draw one inside the hour row where the block starts, in a day or week grid.
   *
   * ```svelte
   * <TimeSlot {hour}>
   *   {#each busyForHour(hour) as block, i (i)}
   *     <BusyBand {block} {hour} />
   *   {/each}
   * </TimeSlot>
   * ```
   *
   * - Absolutely positioned: the parent must be `relative`. The top offset comes
   *   from the block's start minute and the height from its duration (at least
   *   12px), so it can run past its own row.
   * - Not interactive: a `div` with `pointer-events-none`, so clicks reach
   *   whatever is under it.
   * - `aria-label` is "Busy from HH:MM", or "Busy until HH:MM" when the block
   *   started before this day.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/BusyBand',
    component: BusyBand,
    args: { hour: 14, block: block(day(14), day(16)) }
  });
</script>

<!-- Positioned inside an hour row, the way DayView/WeekView draw it. -->
{#snippet hourRow(args: ComponentProps<typeof BusyBand>)}
  <div class="relative h-20 w-64 border-t border-gray-200">
    <BusyBand {...args} />
  </div>
{/snippet}

<!-- A block from 14:00 to 16:00 drawn in the 14:00 row: it runs past the row, two hours tall. -->
<Story name="Two hours" template={hourRow} />

<!-- A block starting at 14:30 is offset half-way down its row. -->
<Story
  name="Starts mid-hour"
  args={{ block: block(day(14, 30), day(15, 15)) }}
  template={hourRow}
/>

<!-- A five-minute block is drawn at the 12px minimum height. -->
<Story
  name="Very short (min height)"
  args={{ block: block(day(14), day(14, 5)) }}
  template={hourRow}
/>

<!-- `clippedStart`: carried over from last night, so it is labelled "until", not "from". -->
<Story
  name="Continues from the day before"
  args={{ hour: 0, block: block(day(0), day(2), true) }}
  template={hourRow}
/>

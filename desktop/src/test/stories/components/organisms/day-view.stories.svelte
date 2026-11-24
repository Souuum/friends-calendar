<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import type { ComponentProps } from 'svelte';
  import DayView from '$lib/components/organisms/DayView.svelte';
  import { BUSY, EVENTS, eventsForDayOf } from '../../fixtures';

  const today = new Date();

  /**
   * The calendar's day view: one day as a 24-hour grid, with events placed at
   * their hours and imported busy time drawn as anonymous bands behind them.
   *
   * ```svelte
   * <DayView
   *   events={eventsForDay(currentDate)}
   *   {busy}
   *   day={currentDate}
   *   onEventClick={selectEvent}
   * />
   * ```
   *
   * - `events` must already be filtered to the day; the component does not
   *   filter. `day` is only used to clip `busy` blocks to that day.
   * - Opens scrolled to the first event rather than to midnight.
   * - A day with neither events nor busy blocks shows "No events today" instead
   *   of the grid.
   * - Dispatches no events: a clicked event calls `onEventClick`.
   */
  const { Story } = defineMeta({
    title: 'Components/Organisms/DayView',
    component: DayView,
    parameters: { layout: 'fullscreen' },
    args: { day: today, events: eventsForDayOf(EVENTS)(today), onEventClick: fn() }
  });
</script>

<!-- Through a snippet: a prop named `events` collides with a Svelte mount option. -->
{#snippet dayView(args: ComponentProps<typeof DayView>)}
  <DayView {...args} />
{/snippet}

<!-- `events` for today on the hour grid, with no `busy` blocks. -->
<Story name="With events" template={dayView} />

<!-- `busy` set: imported calendar time is drawn as bands, clipped to `day`. -->
<Story name="With imported busy blocks" args={{ busy: BUSY }} template={dayView} />

<!-- An empty `events` list and no `busy` blocks: the empty state replaces the grid. -->
<Story name="Empty day" args={{ events: [] }} template={dayView} />

<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import type { ComponentProps } from 'svelte';
  import TimedEvent from '$lib/components/molecules/TimedEvent.svelte';
  import { EVENT_HOSTED, EVENT_LONG_TITLE, at } from '../../fixtures';

  /**
   * An event drawn as a block on an hour grid, showing its title and start time.
   * Draw one inside the hour row where the event starts, in a day or week view.
   *
   * ```svelte
   * <TimeSlot {hour}>
   *   {#each hourEvents as event (event.id)}
   *     <TimedEvent {event} onClick={() => (selected = event)} />
   *   {/each}
   * </TimeSlot>
   * ```
   *
   * - Absolutely positioned: the parent must be `relative`. The start minute
   *   sets the top offset and the duration sets the height, so it can run past
   *   its own row.
   * - The minimum height is 20px, however short the event.
   * - An empty title shows as "Untitled Event".
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/TimedEvent',
    component: TimedEvent,
    args: { event: EVENT_HOSTED, onClick: fn() }
  });
</script>

<!-- Absolutely positioned against its hour row, as in DayView/WeekView. -->
{#snippet hourRow(args: ComponentProps<typeof TimedEvent>)}
  <div class="relative h-20 w-64 border-t border-gray-200">
    <TimedEvent {...args} />
  </div>
{/snippet}

<!-- A two-hour event starting on the hour: twice the height of its row. -->
<Story name="Two hours" template={hourRow} />

<!-- A 20:30 start is offset half-way down the row; 45 minutes long. -->
<Story
  name="Starts at :30"
  args={{ event: { ...EVENT_HOSTED, start_time: at(0, 20, 30), end_time: at(0, 21, 15) } }}
  template={hourRow}
/>

<!-- A long title is cut off with an ellipsis instead of wrapping. -->
<Story name="Long title" args={{ event: EVENT_LONG_TITLE }} template={hourRow} />

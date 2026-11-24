<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import type { ComponentProps } from 'svelte';
  import EventList from '$lib/components/molecules/EventList.svelte';
  import { EVENTS } from '../../fixtures';

  /**
   * A vertical stack of events, cut off after `maxVisible`. Use it where space
   * is fixed, such as a day cell in the month grid.
   *
   * ```svelte
   * <EventList
   *   events={dayEvents}
   *   variant="compact"
   *   maxVisible={3}
   *   showMore={true}
   *   onEventClick={(event) => (selected = event)}
   * />
   * ```
   *
   * - Each row is a `CalendarEvent` drawn with `variant`.
   * - A click calls `onEventClick(event)` and does not propagate, so a clickable
   *   parent (the day cell) is not triggered too.
   * - Events past `maxVisible` are dropped silently unless `showMore` is on.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/EventList',
    component: EventList,
    argTypes: {
      variant: { control: 'inline-radio', options: ['compact', 'default', 'detailed', 'card'] }
    },
    args: { events: EVENTS, onEventClick: fn() }
  });
</script>

<!-- Through a snippet: a prop named `events` collides with a Svelte mount option. -->
{#snippet list(args: ComponentProps<typeof EventList>)}
  <EventList {...args} />
{/snippet}

<!-- The defaults: six events given, the first three drawn, no "+N more" line. -->
<Story name="Default (capped at 3)" template={list} />

<!-- `variant="compact"`, the form the month grid uses. -->
<Story name="Compact" args={{ variant: 'compact' }} template={list} />

<!-- `variant="detailed"`. -->
<Story name="Detailed" args={{ variant: 'detailed' }} template={list} />

<!-- `variant="card"`. -->
<Story name="Card" args={{ variant: 'card' }} template={list} />

<!-- `showMore` adds a "+3 more" line for the events past the cap. -->
<Story name="Show more" args={{ showMore: true }} template={list} />

<!-- `maxVisible={5}` raises the cap, leaving "+1 more". -->
<Story name="Raised cap" args={{ maxVisible: 5, showMore: true }} template={list} />

<!-- An empty `events` list renders an empty container. -->
<Story name="Empty" args={{ events: [] }} template={list} />

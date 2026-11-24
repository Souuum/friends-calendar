<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CalendarEvent from '$lib/components/atoms/CalendarEvent.svelte';
  import { EVENT_CREATOR } from './event/mock';
  import { EVENT_HOSTED } from '../../fixtures';

  /**
   * Renders one event in the view named by `variant`, so a list can switch
   * density without choosing the component itself.
   *
   * ```svelte
   * {#each events as event (event.id)}
   *   <CalendarEvent {event} variant="compact" />
   * {/each}
   * ```
   *
   * `compact` renders `CompactEvent`, `default` renders `Event`, `detailed`
   * renders `DetailedEvent` and `card` renders `EventCard`. Only `event` is
   * passed on, and no events are forwarded: an `on:deleted` from the card
   * view does not reach the caller.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/CalendarEvent',
    component: CalendarEvent,
    argTypes: {
      variant: { control: 'inline-radio', options: ['compact', 'default', 'detailed', 'card'] }
    }
  });
</script>

<!-- No `variant`, so the default one: the small `Event` block with a title and a start time. -->
<Story name="Default" args={{ event: EVENT_CREATOR }} />

<!-- `variant="compact"`: the one-line `CompactEvent` chip of the month grid. -->
<Story name="Compact" args={{ event: EVENT_HOSTED, variant: 'compact' }} />

<!-- `variant="detailed"`: `DetailedEvent`, with the time range and the description. -->
<Story name="Detailed" args={{ event: EVENT_HOSTED, variant: 'detailed' }} />

<!-- `variant="card"`: the full `EventCard`, with participants and actions. -->
<Story name="Card" args={{ event: EVENT_HOSTED, variant: 'card' }} />

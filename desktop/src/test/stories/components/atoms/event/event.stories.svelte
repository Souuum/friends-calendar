<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Event from '$lib/components/atoms/event/Event.svelte';
  import { EVENT_HOSTED, EVENT_LONG_TITLE } from '../../../fixtures';

  /**
   * A small block for one event: the title and the time of day it starts.
   * `CalendarEvent` renders it when no other `variant` is asked for.
   *
   * ```svelte
   * {#each events as event (event.id)}
   *   <Event {event} />
   * {/each}
   * ```
   *
   * It is not interactive and forwards no events. A missing title reads
   * "Untitled Event".
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/Event',
    component: Event,
    args: { event: EVENT_HOSTED }
  });
</script>

<!-- An `event` with a short title and a start time. -->
<Story name="Default" />

<!-- A long title wraps onto several lines instead of being cut off. -->
<Story name="Long title" args={{ event: EVENT_LONG_TITLE }} />

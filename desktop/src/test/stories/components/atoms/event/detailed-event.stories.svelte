<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import DetailedEvent from '$lib/components/atoms/event/DetailedEvent.svelte';
  import { EVENT_HOSTED, EVENT_LONG_TITLE } from '../../../fixtures';

  /**
   * A roomy block for one event: the title, the start and end time of day,
   * and the description, with an accent border on the left. `CalendarEvent`
   * renders it for `variant="detailed"`.
   *
   * ```svelte
   * {#each events as event (event.id)}
   *   <DetailedEvent {event} />
   * {/each}
   * ```
   *
   * It is not interactive and forwards no events. Only the time of day is
   * shown, not the date. A missing title reads "Untitled Event".
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/DetailedEvent',
    component: DetailedEvent,
    args: { event: EVENT_HOSTED }
  });
</script>

<!-- An `event` with a title, a time range and a description. -->
<Story name="Default" />

<!-- A long title wraps onto several lines instead of being cut off. -->
<Story name="Long title" args={{ event: EVENT_LONG_TITLE }} />

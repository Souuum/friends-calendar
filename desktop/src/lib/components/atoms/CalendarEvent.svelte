<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import { getEventView } from '$lib/components/atoms/event';

  /** The event to show, as returned by `GET /api/events`. */
  export let event: EventWithParticipants;
  /**
   * Which view renders it: `compact` -> `CompactEvent`, `default` -> `Event`,
   * `detailed` -> `DetailedEvent`, `card` -> `EventCard`.
   */
  export let variant: 'compact' | 'default' | 'detailed' | 'card' = 'default';
</script>

<svelte:component this={getEventView(variant)} {event} />

<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import { getEventView } from '$lib/components/atoms/event';

  export let event: EventWithParticipants;
  export let variant: 'compact' | 'default' | 'detailed' | 'card' = 'default';
</script>

<svelte:component this={getEventView(variant)} {event} />

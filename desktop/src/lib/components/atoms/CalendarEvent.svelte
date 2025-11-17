<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import EventCard from '$lib/components/EventCard.svelte';
  import { getEventView } from '$lib/components/atoms/event';

  export let event: EventWithParticipants;
  export let variant: 'compact' | 'default' | 'detailed' | 'card' = 'default';
</script>

<svelte:component this={getEventView(variant)} {event} />

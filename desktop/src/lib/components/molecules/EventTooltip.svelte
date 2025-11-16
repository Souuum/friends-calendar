<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import EventCard from '$lib/components/EventCard.svelte';
  import { createEventDispatcher } from 'svelte';

  export let events: EventWithParticipants[];
  export let isVisible = false;
  export let position: { x: number; y: number } = { x: 0, y: 0 };

  const dispatch = createEventDispatcher();

  function handleRefresh() {
    dispatch('refresh');
  }
</script>

{#if isVisible && events.length > 0}
  <div
    class="fixed z-50 pointer-events-auto transition-opacity duration-200"
    class:opacity-0={!isVisible}
    class:opacity-100={isVisible}
    style="left: {position.x}px; top: {position.y}px;"
  >
    <div
      class="bg-white rounded-lg shadow-xl border border-gray-200 p-4 w-96 max-h-[500px] overflow-y-auto transition animate-in fade-in slide-in-from-top-2 duration-200"
    >
      <div class="space-y-3">
        {#each events as event}
          <EventCard {event} on:refresh={handleRefresh} />
        {/each}
      </div>
    </div>
  </div>
{/if}

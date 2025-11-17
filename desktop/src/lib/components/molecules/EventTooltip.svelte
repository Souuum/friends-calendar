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

  function handleMouseEnter() {
    dispatch('mouseenter');
  }

  function handleMouseLeave() {
    dispatch('mouseleave');
  }
</script>

{#if isVisible && events.length > 0}
  <div
    class="fixed z-50 pointer-events-auto transition-opacity duration-200"
    class:opacity-0={!isVisible}
    class:opacity-100={isVisible}
    style="left: {position.x}px; top: {position.y}px;"
    on:mouseenter={handleMouseEnter}
    on:mouseleave={handleMouseLeave}
    role="tooltip"
    tabindex="-1"
  >
    <div
      class="bg-white rounded-lg shadow-xl border border-gray-200 p-4 w-96 max-h-[500px] overflow-y-auto"
    >
      <div class="space-y-3">
        {#each events as event}
          <EventCard {event} on:refresh={handleRefresh} />
        {/each}
      </div>
    </div>
  </div>
{/if}

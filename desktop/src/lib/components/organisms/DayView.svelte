<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';

  export let events: EventWithParticipants[];
</script>

<div class="p-6">
  {#if events.length === 0}
    <div class="text-center py-12 text-gray-500">
      <svg
        class="w-16 h-16 mx-auto mb-4 text-gray-300"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
        />
      </svg>
      <p class="text-lg font-medium">No events today</p>
    </div>
  {:else}
    <div class="space-y-3">
      {#each events as event}
        <slot name="event" {event} />
      {/each}
    </div>
  {/if}
</div>

<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { EventWithParticipants } from '$lib/types';
  import EventCardStatusBar from './EventCardStatusBar.svelte';
  import EventCardParticipant from './EventCardParticipant.svelte';
  import { formatDate } from '$lib/utils/dateUtils';

  export let event: EventWithParticipants;

  const dispatch = createEventDispatcher();

</script>

<div class="bg-white rounded-lg shadow hover:shadow-lg transition p-5">
  <div class="flex justify-between items-start mb-3">
    <h3 class="font-bold text-lg text-gray-900">{event.title}</h3>
    {#if event.is_creator}
      <span class="text-xs bg-discord-blurple text-white px-2 py-1 rounded">Creator</span>
    {/if}
  </div>

  {#if event.description}
    <p class="text-gray-600 text-sm mb-3">{event.description}</p>
  {/if}

  <div class="space-y-2 text-sm text-gray-500 mb-4">
    <div class="flex items-center gap-2">
      <span>🕐</span>
      <span>{formatDate(event.start_time)}</span>
    </div>
    {#if event.location}
      <div class="flex items-center gap-2">
        <span>📍</span>
        <span>{event.location}</span>
      </div>
    {/if}
  </div>

  <EventCardParticipant participants={event.participants}/>

  {#if !event.is_creator && event.my_status}
    <EventCardStatusBar bind:status={event.my_status}/>
  {/if}

  {#if event.is_creator}
    <button
      on:click={() => dispatch("deleted", event)}
      class="w-full mt-2 text-xs py-2 rounded bg-red-100 text-red-700 hover:bg-red-200 cursor-pointer"
    >
      Delete Event
    </button>
  {/if}
</div>

<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import EventCardParticipant from '$lib/components/atoms/event/EventCardParticipant.svelte';
  import { formatDate } from '$lib/utils/dateUtils';

  // Read-only: shows whether you (and everyone else) responded to an
  // event that was posted to the linked Discord channel, it doesn't let
  // you change your RSVP from here - that already exists via
  // EventDetailsModal (opened from the calendar view).
  export let event: EventWithParticipants;

  function statusColor(status?: string) {
    switch (status) {
      case 'accepted':
        return 'bg-green-100 text-green-800';
      case 'declined':
        return 'bg-red-100 text-red-800';
      case 'maybe':
        return 'bg-yellow-100 text-yellow-800';
      default:
        return 'bg-gray-100 text-gray-800';
    }
  }

  function statusLabel(status?: string) {
    switch (status) {
      case 'accepted':
        return '✓ You accepted';
      case 'declined':
        return '✗ You declined';
      case 'maybe':
        return '? You said maybe';
      default:
        return 'No response yet';
    }
  }
</script>

<div class="bg-white rounded-lg shadow p-5 space-y-3">
  <div class="flex justify-between items-start gap-3">
    <h3 class="font-bold text-lg text-gray-900">{event.title}</h3>
    <span class="shrink-0 text-xs px-2 py-1 rounded {statusColor(event.my_status)}">
      {statusLabel(event.my_status)}
    </span>
  </div>

  {#if event.description}
    <p class="text-gray-600 text-sm">{event.description}</p>
  {/if}

  <div class="space-y-1 text-sm text-gray-500">
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
    {#if event.price}
      <div class="flex items-center gap-2">
        <span>💶</span>
        <span>{event.price}</span>
      </div>
    {/if}
    {#if event.link}
      <div class="flex items-center gap-2">
        <span>🔗</span>
        <a href={event.link} target="_blank" rel="noreferrer" class="text-discord-blurple hover:underline">
          {event.link}
        </a>
      </div>
    {/if}
  </div>

  <EventCardParticipant participants={event.participants} />
</div>

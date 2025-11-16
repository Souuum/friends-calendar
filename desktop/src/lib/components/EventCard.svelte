<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { api } from '$lib/api';
  import type { EventWithParticipants } from '$lib/types';

  export let event: EventWithParticipants;

  const dispatch = createEventDispatcher();

  function formatDate(dateString: string) {
    const date = new Date(dateString);
    return date.toLocaleDateString('en-US', {
      weekday: 'short',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  function getStatusColor(status: string) {
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

  async function handleStatusChange(status: 'accepted' | 'declined' | 'maybe') {
    try {
      await api.updateParticipation(event.id, status);
      dispatch('refresh');
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to update status');
    }
  }

  async function handleDelete() {
    if (confirm('Are you sure you want to delete this event?')) {
      try {
        await api.deleteEvent(event.id);
        dispatch('refresh');
      } catch (err) {
        alert(err instanceof Error ? err.message : 'Failed to delete event');
      }
    }
  }
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

  <div class="mb-4">
    <p class="text-xs font-semibold text-gray-500 mb-2">
      {event.participants.length} participant{event.participants.length !== 1 ? 's' : ''}
    </p>
    <div class="flex flex-wrap gap-2">
      {#each event.participants.slice(0, 5) as participant}
        <div class="flex items-center gap-1">
          {#if participant.avatar_url}
            <img
              src={participant.avatar_url}
              alt={participant.username}
              class="w-6 h-6 rounded-full"
            />
          {:else}
            <div class="w-6 h-6 rounded-full bg-gray-300"></div>
          {/if}
          <span class="text-xs {getStatusColor(participant.status)} px-2 py-0.5 rounded">
            {participant.username}
          </span>
        </div>
      {/each}
      {#if event.participants.length > 5}
        <span class="text-xs text-gray-500">+{event.participants.length - 5} more</span>
      {/if}
    </div>
  </div>

  {#if !event.is_creator && event.my_status}
    <div class="flex gap-2">
      <button
        on:click={() => handleStatusChange('accepted')}
        class="flex-1 text-xs py-2 rounded {event.my_status === 'accepted'
          ? 'bg-green-500 text-white'
          : 'bg-gray-200 text-gray-700'}"
      >
        ✓ Accept
      </button>
      <button
        on:click={() => handleStatusChange('maybe')}
        class="flex-1 text-xs py-2 rounded {event.my_status === 'maybe'
          ? 'bg-yellow-500 text-white'
          : 'bg-gray-200 text-gray-700'}"
      >
        ? Maybe
      </button>
      <button
        on:click={() => handleStatusChange('declined')}
        class="flex-1 text-xs py-2 rounded {event.my_status === 'declined'
          ? 'bg-red-500 text-white'
          : 'bg-gray-200 text-gray-700'}"
      >
        ✗ Decline
      </button>
    </div>
  {/if}

  {#if event.is_creator}
    <button
      on:click={handleDelete}
      class="w-full mt-2 text-xs py-2 rounded bg-red-100 text-red-700 hover:bg-red-200"
    >
      Delete Event
    </button>
  {/if}
</div>

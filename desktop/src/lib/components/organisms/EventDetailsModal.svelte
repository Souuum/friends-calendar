<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { api } from '$lib/api';
  import type { EventWithParticipants } from '$lib/types';
  import BlurModal from './BlurModal.svelte';

  export let event: EventWithParticipants | null;
  export let isOpen = false;

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
    if (!event) return;
    try {
      await api.updateParticipation(event.id, status);
      dispatch('refresh');
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to update status');
    }
  }

  async function handleDelete() {
    if (!event) return;
    if (confirm('Are you sure you want to delete this event?')) {
      try {
        await api.deleteEvent(event.id);
        close();
        dispatch('refresh');
      } catch (err) {
        alert(err instanceof Error ? err.message : 'Failed to delete event');
      }
    }
  }

  function close() {
    dispatch('close');
  }
</script>

<BlurModal {isOpen} size="lg" onClose={close} blurAmount="sm" overlayOpacity="dark">
  {#if event}
    <div class="flex justify-between items-start p-6 border-b bg-white">
      <div class="flex-1">
        <div class="flex items-center gap-2 mb-2">
          <h2 class="font-bold text-2xl text-gray-900">{event.title}</h2>
          {#if event.is_creator}
            <span class="text-xs bg-discord-blurple text-white px-2 py-1 rounded">Creator</span>
          {/if}
        </div>
        {#if event.description}
          <p class="text-gray-600">{event.description}</p>
        {/if}
      </div>
      <button
        on:click={close}
        class="ml-4 p-2 hover:bg-gray-100 rounded-lg transition-colors shrink-0"
        aria-label="Close"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M6 18L18 6M6 6l12 12"
          />
        </svg>
      </button>
    </div>

    <div class="overflow-y-auto max-h-[calc(90vh-80px)]">
      <div class="p-6 space-y-6">
        <!-- Event details -->
        <div class="space-y-3 text-gray-700">
          <div class="flex items-center gap-3">
            <span class="text-2xl">🕐</span>
            <div>
              <p class="font-medium">Start Time</p>
              <p class="text-sm text-gray-600">{formatDate(event.start_time)}</p>
            </div>
          </div>
          {#if event.end_time}
            <div class="flex items-center gap-3">
              <span class="text-2xl">⏰</span>
              <div>
                <p class="font-medium">End Time</p>
                <p class="text-sm text-gray-600">{formatDate(event.end_time)}</p>
              </div>
            </div>
          {/if}
          {#if event.location}
            <div class="flex items-center gap-3">
              <span class="text-2xl">📍</span>
              <div>
                <p class="font-medium">Location</p>
                <p class="text-sm text-gray-600">{event.location}</p>
              </div>
            </div>
          {/if}
        </div>

        <div>
          <p class="font-semibold text-gray-900 mb-3">
            Participants ({event.participants.length})
          </p>
          <div class="space-y-2">
            {#each event.participants as participant}
              <div class="flex items-center gap-3 p-2 rounded-lg hover:bg-gray-50">
                {#if participant.avatar_url}
                  <img
                    src={participant.avatar_url}
                    alt={participant.username}
                    class="w-10 h-10 rounded-full"
                  />
                {:else}
                  <div class="w-10 h-10 rounded-full bg-gray-300"></div>
                {/if}
                <div class="flex-1">
                  <p class="font-medium text-gray-900">{participant.username}</p>
                </div>
                <span class="text-xs {getStatusColor(participant.status)} px-3 py-1 rounded-full">
                  {participant.status}
                </span>
              </div>
            {/each}
          </div>
        </div>

        {#if !event.is_creator && event.my_status}
          <div class="border-t pt-6">
            <p class="font-semibold text-gray-900 mb-3">Your Response</p>
            <div class="flex gap-3">
              <button
                on:click={() => handleStatusChange('accepted')}
                class="flex-1 py-3 rounded-lg font-medium transition-colors {event.my_status ===
                'accepted'
                  ? 'bg-green-500 text-white'
                  : 'bg-gray-100 text-gray-700 hover:bg-gray-200'}"
              >
                ✓ Accept
              </button>
              <button
                on:click={() => handleStatusChange('maybe')}
                class="flex-1 py-3 rounded-lg font-medium transition-colors {event.my_status ===
                'maybe'
                  ? 'bg-yellow-500 text-white'
                  : 'bg-gray-100 text-gray-700 hover:bg-gray-200'}"
              >
                ? Maybe
              </button>
              <button
                on:click={() => handleStatusChange('declined')}
                class="flex-1 py-3 rounded-lg font-medium transition-colors {event.my_status ===
                'declined'
                  ? 'bg-red-500 text-white'
                  : 'bg-gray-100 text-gray-700 hover:bg-gray-200'}"
              >
                ✗ Decline
              </button>
            </div>
          </div>
        {/if}

        {#if event.is_creator}
          <div class="border-t pt-6">
            <button
              on:click={handleDelete}
              class="w-full py-3 rounded-lg bg-red-100 text-red-700 hover:bg-red-200 font-medium transition-colors"
            >
              Delete Event
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</BlurModal>

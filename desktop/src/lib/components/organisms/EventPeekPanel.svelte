<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { api } from '$lib/api';
  import type { EventWithParticipants } from '$lib/types';
  import { formatDate } from '$lib/utils/dateUtils';

  // Persistent side panel replacing the old hover-tooltip (month) /
  // modal-on-click (week, day) interaction - see
  // .claude/skills/mockup-calendar-redesign/SKILL.md. Presentational
  // aside from data-fetching a friend's info; the RSVP call itself mirrors
  // EventDetailsModal.svelte's handleStatusChange exactly, so both stay
  // in sync if the status set ever changes.
  export let event: EventWithParticipants | null = null;

  const dispatch = createEventDispatcher();

  let updating = false;
  let error = '';

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

  async function handleStatusChange(status: 'accepted' | 'declined' | 'maybe') {
    if (!event) return;
    try {
      updating = true;
      error = '';
      await api.updateParticipation(event.id, status);
      dispatch('refresh');
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to update status';
    } finally {
      updating = false;
    }
  }
</script>

<aside class="w-[296px] shrink-0 bg-white border border-gray-200 rounded-xl p-4">
  {#if !event}
    <p class="text-sm text-gray-500">Select an event to see its details here.</p>
  {:else}
    <div class="flex items-center gap-2 mb-2 text-xs text-gray-500">
      <span>{formatDate(event.start_time)}</span>
      {#if event.my_status}
        <span class="px-2 py-0.5 rounded-full {statusColor(event.my_status)}">{event.my_status}</span>
      {/if}
    </div>
    <h2 class="text-lg font-semibold text-gray-900 mb-2">{event.title}</h2>
    <div class="flex flex-col gap-1 text-xs text-gray-600 mb-3">
      {#if event.location}
        <div>{event.location}</div>
      {/if}
      {#if event.price}
        <div>Cost per person: {event.price}</div>
      {/if}
    </div>

    {#if error}
      <p class="text-xs text-red-600 mb-2" role="alert">{error}</p>
    {/if}

    {#if !event.is_creator}
      <div class="flex gap-1.5 mb-4">
        <button
          on:click={() => handleStatusChange('accepted')}
          disabled={updating}
          class="flex-1 py-2 rounded-lg text-xs font-semibold disabled:opacity-50 {event.my_status ===
          'accepted'
            ? 'bg-primary text-white'
            : 'bg-gray-100 text-gray-700 hover:bg-gray-200'}"
        >
          Going
        </button>
        <button
          on:click={() => handleStatusChange('maybe')}
          disabled={updating}
          class="flex-1 py-2 rounded-lg text-xs font-semibold disabled:opacity-50 {event.my_status ===
          'maybe'
            ? 'bg-yellow-500 text-white'
            : 'bg-gray-100 text-gray-700 hover:bg-gray-200'}"
        >
          Maybe
        </button>
        <button
          on:click={() => handleStatusChange('declined')}
          disabled={updating}
          class="flex-1 py-2 rounded-lg text-xs font-semibold disabled:opacity-50 {event.my_status ===
          'declined'
            ? 'bg-red-500 text-white'
            : 'bg-gray-100 text-gray-700 hover:bg-gray-200'}"
        >
          Can't
        </button>
      </div>
    {/if}

    <div class="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-2">
      {event.participants.length} invited
    </div>
    <div class="flex flex-col gap-2">
      {#each event.participants as participant}
        <div class="flex items-center gap-2">
          {#if participant.avatar_url}
            <img src={participant.avatar_url} alt="" class="w-7 h-7 rounded-full" />
          {:else}
            <div class="w-7 h-7 rounded-full bg-gray-300"></div>
          {/if}
          <span class="flex-1 text-sm text-gray-800 truncate">{participant.username}</span>
          <span class="text-[11px] px-2 py-0.5 rounded-full {statusColor(participant.status)}">
            {participant.status}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</aside>

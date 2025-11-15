<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib//api';
  import type { EventWithParticipants } from '$lib/types';
  import { user } from '$lib/stores';
  import EventCard from '$lib/components/EventCard.svelte';
  import CreateEventModal from '$lib/components/CreateEventModal.svelte';

  let events: EventWithParticipants[] = [];
  let loading = true;
  let error = '';
  let showCreateModal = false;

  async function loadEvents() {
    try {
      loading = true;
      events = await api.getEvents();
      events.sort((a, b) => new Date(a.start_time).getTime() - new Date(b.start_time).getTime());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load events';
    } finally {
      loading = false;
    }
  }

  function handleEventCreated() {
    showCreateModal = false;
    loadEvents();
  }

  function handleLogout() {
    api.clearToken();
    window.location.reload();
  }

  onMount(loadEvents);
</script>

<div class="min-h-screen bg-gray-50">
  <!-- Header -->
  <header class="bg-white shadow">
    <div class="max-w-7xl mx-auto px-4 py-4 sm:px-6 lg:px-8 flex justify-between items-center">
      <div class="flex items-center gap-4">
        <h1 class="text-2xl font-bold text-gray-900">📅 My Calendar</h1>
        {#if $user}
          <div class="flex items-center gap-2">
            {#if $user.avatar_url}
              <img src={$user.avatar_url} alt="Avatar" class="w-8 h-8 rounded-full" />
            {/if}
            <span class="text-sm text-gray-600">{$user.username}</span>
          </div>
        {/if}
      </div>
      <div class="flex gap-2">
        <button
          on:click={() => showCreateModal = true}
          class="bg-discord-blurple hover:bg-blue-600 text-white px-4 py-2 rounded-lg font-medium transition"
        >
          + New Event
        </button>
        <button
          on:click={handleLogout}
          class="bg-gray-200 hover:bg-gray-300 text-gray-700 px-4 py-2 rounded-lg font-medium transition"
        >
          Logout
        </button>
      </div>
    </div>
  </header>

  <!-- Content -->
  <main class="max-w-7xl mx-auto px-4 py-8 sm:px-6 lg:px-8">
    {#if loading}
      <div class="text-center py-12">
        <div class="inline-block animate-spin rounded-full h-12 w-12 border-b-2 border-discord-blurple"></div>
        <p class="mt-4 text-gray-600">Loading events...</p>
      </div>
    {:else if error}
      <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded">
        {error}
      </div>
    {:else if events.length === 0}
      <div class="text-center py-12">
        <p class="text-gray-600 text-lg">No events yet</p>
        <p class="text-gray-500 mt-2">Create your first event to get started!</p>
      </div>
    {:else}
      <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        {#each events as event (event.id)}
          <EventCard {event} on:refresh={loadEvents} />
        {/each}
      </div>
    {/if}
  </main>
</div>

{#if showCreateModal}
  <CreateEventModal
    on:close={() => showCreateModal = false}
    on:created={handleEventCreated}
  />
{/if}
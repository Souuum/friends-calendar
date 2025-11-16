<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib//api';
  import type { EventWithParticipants } from '$lib/types';
  import { user } from '$lib/stores';
  import Calendar from '$lib/components/Calendar.svelte';
  import EventCard from '$lib/components/EventCard.svelte';
  import CreateEventModal from '$lib/components/CreateEventModal.svelte';
  import Header from '$lib/components/organisms/Header.svelte';

  let events: EventWithParticipants[] = [];
  let loading = true;
  let error = '';
  let showCreateModal = false;
  let avatar_url  = `https://cdn.discordapp.com/avatars/${$user?.discord_id}/${$user?.avatar}.png`

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
  <!-- <header class="bg-white shadow">
    <div class=" mx-auto px-4 py-4 sm:px-6 lg:px-8 flex justify-between items-center">
      <div class="flex items-center gap-4">
        <h1 class="text-2xl font-bold text-gray-900">📅 My Calendar</h1>

      </div>
      <div class="flex gap-2">
        <button
          on:click={() => showCreateModal = true}
          class="bg-discord-blurple hover:bg-blue-600 text-white px-4 py-2 rounded-lg font-medium transition"
        >
          + New Event
        </button>
        </div>
        <div>
        {#if $user}
          <div class="flex items-center gap-2 ">
            {#if avatar_url}
              <img src={avatar_url} alt="Avatar" class="w-12 h-12 rounded-full p-1" />
            {/if}
            <span class="p-1 text-black font-bold text-xl">{$user.username}</span>
            <button
                class="p-1 hover:bg-gray-100 rounded-lg transition"
                title="Menu"
            >
                <svg viewBox="0 0 24 24" width="24" height="24" xmlns="http://www.w3.org/2000/svg" aria-hidden=true>
                    <path d="M6 9 L12 15 L18 9" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
            </button>
          </div>
        {/if}

      </div>
    </div>
  </header> -->
  {#if $user}
  <Header {avatar_url} {user} on:logout={handleLogout} />
  {/if}

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
    <Calendar {events}>
         <svelte:fragment slot="event" let:event>
            <EventCard {event} on:refresh={loadEvents} />
        </svelte:fragment>
    </Calendar>
    {/if}

  </main>
</div>

{#if showCreateModal}
  <CreateEventModal
    on:close={() => showCreateModal = false}
    on:created={handleEventCreated}
  />
{/if}
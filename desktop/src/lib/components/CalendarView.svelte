<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib//api';
  import type { EventWithParticipants } from '$lib/types';
  import { user } from '$lib/stores';
  import Calendar from '$lib/components/templates/Calendar.svelte';
  import EventCard from '$lib/components/EventCard.svelte';
  import CreateEventModal from '$lib/components/CreateEventModal.svelte';
  import Header from '$lib/components/organisms/Header.svelte';
  import { page } from '$app/stores';

  let events: EventWithParticipants[] = [];
  let loading = true;
  let error = '';
  let showCreateModal = false;
  let avatar_url = `https://cdn.discordapp.com/avatars/${$user?.discord_id}/${$user?.avatar}.png`;

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

  const navItems = [
    { label: 'Calendars', icon: '📅', href: '/calendar' },
    { label: 'Announcement', icon: '🔔', href: '/announcements' },
    { label: 'Some feature', icon: '🪶', href: '/feature' }
  ];

  onMount(loadEvents);
</script>

    <main class=" py-8 sm:px-6 lg:px-8 rounded-xl">
      {#if loading}
        <div class="text-center py-12">
          <div
            class="inline-block animate-spin rounded-full h-12 w-12 border-b-2 border-discord-blurple"
          ></div>
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
          <!-- <svelte:fragment slot="event" let:eventt>
            <EventCard {event} on:refresh={loadEvents} />
          </svelte:fragment> -->
        </Calendar>
      {/if}
    </main>

{#if showCreateModal}
  <CreateEventModal
    on:close={() => showCreateModal = false}
    on:created={handleEventCreated}
  />
{/if}
<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { EventWithParticipants } from '$lib/types';
  import Calendar from '$lib/components/templates/Calendar.svelte';

  let events: EventWithParticipants[] = [];
  let loading = true;
  let error = '';

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

  onMount(loadEvents);
</script>

<main class=" py-4 rounded-xl">
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
  {:else}
    <!--
      The calendar renders whether or not there are events. It used to be
      replaced wholesale by an "No events yet" message, which was a dead
      end: the "+ New Event" button lives in CalendarHeader, inside
      Calendar - so an account with no events had no way to create one and
      stayed empty permanently. A fresh deployment landed in exactly that
      state.

      The empty hint now sits above the grid instead of instead of it.
    -->
    {#if events.length === 0}
      <p class="text-center text-sm text-gray-500 mb-2">
        No events yet — create your first one with “+ New Event”.
      </p>
    {/if}
    <div class="bg-white rounded-lg shadow-sm relative">
      <Calendar {events} on:refresh={loadEvents} />
    </div>
  {/if}
</main>

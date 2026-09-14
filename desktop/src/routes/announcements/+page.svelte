<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import AnnouncementCard from '$lib/components/molecules/AnnouncementCard.svelte';
  import type { EventWithParticipants } from '$lib/types';

  let announcements: EventWithParticipants[] = [];
  let loading = true;
  let error = '';

  async function load() {
    try {
      loading = true;
      error = '';
      // include_declined: this page lists everything that was announced,
      // not just events you're still going to - an event you declined
      // should still show up here.
      const events = await api.getEvents({ include_declined: true });
      announcements = events
        .filter((event) => !!event.discord_message_id)
        .sort((a, b) => new Date(b.start_time).getTime() - new Date(a.start_time).getTime());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load announcements';
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

<svelte:head>
  <title>Announcements - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl mx-auto py-6 px-4 space-y-4">
    <h1 class="text-xl font-semibold">Announcements</h1>

    {#if error}
      <p class="text-sm text-red-600" role="alert">{error}</p>
    {:else if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if announcements.length === 0}
      <p class="text-sm text-gray-500">No events have been announced to Discord yet.</p>
    {:else}
      {#each announcements as event (event.id)}
        <AnnouncementCard {event} />
      {/each}
    {/if}
  </div>
</Frame>

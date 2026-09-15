<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import AnnouncementCard from '$lib/components/molecules/AnnouncementCard.svelte';
  import type { EventWithParticipants, FriendInfo } from '$lib/types';

  // No dedicated GET /api/friends/:id endpoint - the friends list already
  // has everything the header needs, and "shared events" is a filter over
  // GET /api/events. Deliberately omits the mockup's "Mutual servers" card
  // (this app links exactly one Discord server, so every friend's page
  // would show the identical card - not informational) and the "Free this
  // week" availability strip (belongs to the mockup-availability skill).
  // See mockup-friends-directory skill for the full rationale.

  let friend: FriendInfo | undefined;
  let sharedEvents: EventWithParticipants[] = [];
  let loading = true;
  let error = '';

  $: friendId = $page.params.id;

  async function load(id: string) {
    try {
      loading = true;
      error = '';
      const [friends, events] = await Promise.all([
        api.getFriends(),
        api.getEvents({ include_declined: true })
      ]);
      friend = friends.find((f) => f.user_id === id);
      sharedEvents = events
        .filter((e) => e.participants.some((p) => p.user_id === id))
        .sort((a, b) => new Date(b.start_time).getTime() - new Date(a.start_time).getTime());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load friend';
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (friendId) load(friendId);
  });
</script>

<svelte:head>
  <title>{friend ? friend.username : 'Friend'} - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl mx-auto py-6 px-4">
    <button
      class="text-sm text-discord-blurple hover:underline mb-4 bg-transparent border-none cursor-pointer p-0"
      on:click={() => goto('/friends')}
    >
      ← Back to friends
    </button>

    {#if error}
      <p class="text-sm text-red-600" role="alert">{error}</p>
    {:else if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if !friend}
      <p class="text-sm text-gray-500">Friend not found.</p>
    {:else}
      <div class="flex items-center gap-4 mb-6">
        {#if friend.avatar_url}
          <img src={friend.avatar_url} alt="" class="w-16 h-16 rounded-full" />
        {:else}
          <div class="w-16 h-16 rounded-full bg-gray-300"></div>
        {/if}
        <h1 class="text-2xl font-semibold m-0">{friend.username}</h1>
      </div>

      <h2 class="text-lg font-semibold mb-3">Shared events</h2>
      {#if sharedEvents.length === 0}
        <p class="text-sm text-gray-500">No shared events yet.</p>
      {:else}
        <div class="space-y-3">
          {#each sharedEvents as event (event.id)}
            <AnnouncementCard {event} />
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</Frame>

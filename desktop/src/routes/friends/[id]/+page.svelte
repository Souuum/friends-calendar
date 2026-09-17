<script lang="ts">
  import Icon from '$lib/components/atoms/Icon.svelte';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import EventRsvpCard from '$lib/components/molecules/EventRsvpCard.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import type { DayAvailability, EventWithParticipants, FriendInfo } from '$lib/types';

  // No dedicated GET /api/friends/:id endpoint - the friends list already
  // has everything the header needs, and "shared events" is a filter over
  // GET /api/events. Deliberately omits the mockup's "Mutual servers" card
  // (this app links exactly one Discord server, so every friend's page
  // would show the identical card - not informational).
  // See mockup-friends-directory skill for the full rationale.

  let friend: FriendInfo | undefined;
  let sharedEvents: EventWithParticipants[] = [];
  let loading = true;
  let error = '';

  let availability: DayAvailability[] = [];

  $: friendId = $page.params.id;

  const WEEKDAY_LABELS = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];

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
        // `is_participant` matters since GET /api/events started returning
        // events you can merely see (public, or a friend's friends-visible
        // event). Without it, "Shared events" would list events only *they*
        // are in, which is the opposite of shared.
        .filter((e) => e.is_participant && e.participants.some((p) => p.user_id === id))
        .sort((a, b) => new Date(b.start_time).getTime() - new Date(a.start_time).getTime());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load friend';
    } finally {
      loading = false;
    }

    // Best-effort, separate from the main load - the friend page is still
    // useful without it (e.g. the availability call requires being
    // actual friends, which we already know is true if we got here).
    try {
      const weekStart = dateUtils.startOfWeek(new Date()).toISOString();
      availability = await api.getWeekAvailability(id, weekStart);
    } catch {
      availability = [];
    }
  }

  function fillClass(freeCount: number): string {
    if (freeCount >= 2) return 'bg-green-300';
    if (freeCount === 1) return 'bg-yellow-100';
    return 'bg-gray-200';
  }

  onMount(() => {
    if (friendId) load(friendId);
  });
</script>

<svelte:head>
  <title>{friend ? friend.username : 'Friend'} - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl mx-auto py-6 px-3 sm:px-4 anim-fade-up">
    <button
      class="inline-flex items-center gap-1 text-sm text-discord-blurple hover:underline mb-4 bg-transparent border-none cursor-pointer p-0"
      on:click={() => goto('/friends')}
    >
      <Icon name="back" size={14} /> Back to friends
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

      {#if availability.length === 7}
        <div class="bg-surface border border-gray-200 rounded-xl p-4 mb-6">
          <h2 class="text-sm font-semibold mb-1">Free this week</h2>
          <p class="text-xs text-gray-500 mb-3">Overlap with your calendar</p>
          <div class="flex gap-1.5">
            {#each availability as day, i (day.date)}
              <div class="flex-1 text-center">
                <div class="font-mono text-[10px] text-gray-400 mb-1">{WEEKDAY_LABELS[i]}</div>
                <div
                  class="h-12 rounded {fillClass(day.free_user_ids.length)}"
                  title={`${day.free_user_ids.length} of 2 free`}
                ></div>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <h2 class="text-lg font-semibold mb-3">Shared events</h2>
      {#if sharedEvents.length === 0}
        <p class="text-sm text-gray-500">No shared events yet.</p>
      {:else}
        <div class="space-y-3">
          {#each sharedEvents as event (event.id)}
            <EventRsvpCard {event} />
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</Frame>

<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import type { FriendInfo, EventWithParticipants } from '$lib/types';

  let friends: FriendInfo[] = [];
  let events: EventWithParticipants[] = [];
  let freeNowIds = new Set<string>();
  let loading = true;
  let error = '';
  let search = '';
  let syncing = false;

  // For each friend, the note is derived from what we actually know
  // (shared upcoming events) rather than the mockup's richer status pills
  // (Going/Maybe/Free Sat/Idle/New) - those need availability data this
  // tier doesn't have. See mockup-friends-directory skill.
  function noteFor(friendId: string): string {
    const shared = events
      // Same reason as /friends/[id]: the event list now includes events
      // you can see but aren't part of, which aren't "shared".
      .filter((e) => e.is_participant && e.participants.some((p) => p.user_id === friendId))
      .filter((e) => new Date(e.start_time).getTime() >= Date.now())
      .sort((a, b) => new Date(a.start_time).getTime() - new Date(b.start_time).getTime());

    if (shared.length === 0) return 'No shared events';
    const next = shared[0];
    const when = new Date(next.start_time).toLocaleDateString('en-US', { weekday: 'short' });
    return `Next: ${next.title}, ${when}`;
  }

  async function load() {
    try {
      loading = true;
      error = '';
      [friends, events] = await Promise.all([
        api.getFriends(),
        api.getEvents({ include_declined: true })
      ]);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load friends';
    } finally {
      loading = false;
    }

    // Best-effort, separate from the main load - a failure here (e.g. no
    // events at all yet) shouldn't block showing the friend list itself.
    try {
      freeNowIds = new Set(await api.getFreeFriendsNow());
    } catch {
      // leave freeNowIds empty - just means no "Free now" pills show up.
    }
  }

  async function handleSync() {
    try {
      syncing = true;
      error = '';
      const result = await api.syncFriends();
      friends = result.friends;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to sync friends';
    } finally {
      syncing = false;
    }
  }

  onMount(load);

  $: visibleFriends = friends.filter((f) =>
    f.username.toLowerCase().includes(search.trim().toLowerCase())
  );
</script>

<svelte:head>
  <title>Friends - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-4xl mx-auto py-6 px-4 anim-fade-up">
    <div class="flex items-end gap-4 flex-wrap mb-5">
      <div>
        <h1 class="text-2xl font-semibold m-0">Friends</h1>
        <p class="text-sm text-gray-500 m-0 mt-1">{friends.length} synced from Discord</p>
      </div>
      <div class="ml-auto flex gap-2">
        <button
          on:click={handleSync}
          disabled={syncing}
          class="px-4 py-2 border border-gray-300 rounded-lg text-sm font-semibold disabled:opacity-50"
        >
          {syncing ? 'Syncing…' : 'Sync friends'}
        </button>
        <button
          on:click={() => goto('/friends/add')}
          class="px-4 py-2 bg-primary text-white rounded-lg text-sm font-semibold"
        >
          + Add friend
        </button>
      </div>
    </div>

    <div class="mb-4">
      <input
        type="text"
        bind:value={search}
        placeholder="Search by name"
        class="w-full max-w-sm px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
      />
    </div>

    {#if error}
      <p class="text-sm text-red-600" role="alert">{error}</p>
    {:else if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if visibleFriends.length === 0}
      <p class="text-sm text-gray-500">
        {friends.length === 0 ? 'No friends synced yet.' : 'No friends match your search.'}
      </p>
    {:else}
      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
        {#each visibleFriends as friend, i (friend.user_id)}
          <a
            href={`/friends/${friend.user_id}`}
            style="animation-delay: {i * 45}ms"
            class="block text-left bg-white border border-gray-200 rounded-xl p-4 transition no-underline anim-fade-up-stagger hover:border-primary hover:shadow-[0_6px_18px_rgba(80,48,229,0.12)] hover:-translate-y-0.5"
          >
            <div class="flex items-center gap-3 mb-3">
              {#if friend.avatar_url}
                <img src={friend.avatar_url} alt="" class="w-10 h-10 rounded-full" />
              {:else}
                <div class="w-10 h-10 rounded-full bg-gray-300"></div>
              {/if}
              <span class="font-semibold text-gray-900 truncate flex-1">{friend.username}</span>
              {#if freeNowIds.has(friend.user_id)}
                <span class="text-xs font-semibold text-green-700 bg-green-100 rounded-full px-2 py-0.5 whitespace-nowrap">
                  Free now
                </span>
              {/if}
            </div>
            <div class="text-xs text-gray-500 border-t border-gray-100 pt-2 truncate">
              {noteFor(friend.user_id)}
            </div>
          </a>
        {/each}
      </div>
    {/if}
  </div>
</Frame>

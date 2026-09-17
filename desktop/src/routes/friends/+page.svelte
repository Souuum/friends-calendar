<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import Icon from '$lib/components/atoms/Icon.svelte';
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
  <div class="anim-fade-up">
    <div class="flex items-end gap-4 flex-wrap mb-5">
      <div>
        <h1 class="text-2xl font-semibold m-0">Friends</h1>
        <p class="text-sm text-gray-500 m-0 mt-1">{friends.length} synced from Discord</p>
      </div>
      <div class="w-full md:w-auto md:ml-auto flex gap-2">
        <button
          on:click={handleSync}
          disabled={syncing}
          class="flex-1 rounded-[9px] border border-line bg-surface px-3.5 py-[9px] text-[13px] font-semibold hover:bg-subtle disabled:opacity-50 md:flex-none"
        >
          {syncing ? 'Syncing…' : 'Sync friends'}
        </button>
        <button
          on:click={() => goto('/friends/add')}
          class="flex-1 rounded-[9px] bg-primary px-3.5 py-[9px] text-[13px] font-semibold text-white hover:bg-primary-active md:flex-none"
        >
          Add friend
        </button>
      </div>
    </div>

    <!-- The mockup's search row: a bordered field carrying its own magnifier
         rather than a bare input, growing to fill the row. -->
    <div class="mb-[18px] flex flex-wrap gap-2">
      <div
        class="flex min-w-[200px] flex-1 items-center gap-2 rounded-[9px] border border-line bg-surface px-3 py-[9px] focus-within:border-primary"
      >
        <Icon name="search" size={14} class="shrink-0 text-muted" />
        <input
          type="text"
          bind:value={search}
          placeholder="Search by name or Discord tag"
          aria-label="Search friends"
          class="min-w-0 flex-1 border-none bg-transparent text-[13px] outline-none placeholder:text-muted"
        />
      </div>
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
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-[repeat(auto-fill,minmax(250px,1fr))]">
        {#each visibleFriends as friend, i (friend.user_id)}
          <a
            href={`/friends/${friend.user_id}`}
            style="animation-delay: {i * 45}ms"
            class="flex flex-col gap-3 rounded-[13px] border border-line bg-surface p-[15px] text-left no-underline transition anim-fade-up-stagger hover:border-primary hover:shadow-[0_6px_18px_rgba(80,48,229,0.12)] hover:-translate-y-0.5"
          >
            <!-- 42px avatar carrying its own presence dot, name over a mono
                 handle - the mockup's card head. -->
            <div class="flex items-center gap-[11px]">
              <div class="relative shrink-0">
                {#if friend.avatar_url}
                  <img src={friend.avatar_url} alt="" class="h-[42px] w-[42px] rounded-full" />
                {:else}
                  <div class="h-[42px] w-[42px] rounded-full bg-line"></div>
                {/if}
                <!-- The dot means "free right now", which is the only presence
                     this app actually knows - it is not Discord's online
                     status, and inventing one would be a lie. -->
                <span
                  class="absolute -bottom-px -right-px h-3 w-3 rounded-full border-2 border-surface {freeNowIds.has(
                    friend.user_id
                  )
                    ? 'bg-primary'
                    : 'bg-line'}"
                  title={freeNowIds.has(friend.user_id) ? 'Free right now' : 'Busy or unknown'}
                ></span>
              </div>
              <div class="min-w-0">
                <div class="truncate text-[14px] font-semibold">{friend.username}</div>
                <div class="truncate font-mono text-[11px] text-muted">
                  synced {new Date(friend.synced_at).toLocaleDateString()}
                </div>
              </div>
            </div>
            <!-- Footer: the shared-event note, and the status pill on the right. -->
            <div class="flex items-center gap-2 border-t border-subtle pt-2.5">
              <span class="min-w-0 flex-1 truncate text-[12px] text-muted">
                {noteFor(friend.user_id)}
              </span>
              {#if freeNowIds.has(friend.user_id)}
                <!-- Tint/accent, not green: the mockup has no green anywhere,
                     and "available" is the same affirmative as "Going". -->
                <span
                  class="shrink-0 whitespace-nowrap rounded-full bg-tint px-2.5 py-1 text-[11px] font-semibold text-primary"
                >
                  Free now
                </span>
              {/if}
            </div>
          </a>
        {/each}
      </div>
    {/if}
  </div>
</Frame>

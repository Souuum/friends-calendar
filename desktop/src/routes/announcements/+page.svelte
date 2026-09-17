<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import AnnouncementPostCard from '$lib/components/molecules/AnnouncementPostCard.svelte';
  import AdoptEventModal from '$lib/components/organisms/AdoptEventModal.svelte';
  import type { AnnouncementPostInfo } from '$lib/types';

  let posts: AnnouncementPostInfo[] = [];
  let loading = true;
  let error = '';
  let syncing = false;

  // Which post is being turned into an event. Null = the modal is closed;
  // one nullable value rather than a separate boolean, so the two can't
  // disagree about what's on screen.
  let adopting: AnnouncementPostInfo | null = null;
  let adoptedMessage = '';

  async function handleAdopted(rsvps: number) {
    adopting = null;
    // Said plainly, because recovering the existing ✅ is the reason to
    // adopt rather than re-create: if nobody had reacted, say that too
    // instead of reporting a bare success and leaving it ambiguous.
    adoptedMessage =
      rsvps > 0
        ? `Added to your calendar, with ${rsvps} ${rsvps === 1 ? 'person' : 'people'} already going.`
        : 'Added to your calendar. Reactions on the post will now count as RSVPs.';
    // Reload so the post shows its new "Event" tag rather than going stale
    // until the next sync.
    await load();
  }

  async function load() {
    try {
      loading = true;
      error = '';
      posts = await api.getAnnouncements();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load announcements';
    } finally {
      loading = false;
    }
  }

  async function handleSync() {
    try {
      syncing = true;
      error = '';
      posts = await api.syncAnnouncements();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to sync announcements';
    } finally {
      syncing = false;
    }
  }

  onMount(load);
</script>

<svelte:head>
  <title>Announcements - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl space-y-4 anim-fade-up">
    <div class="flex items-center justify-between">
      <h1 class="text-xl font-semibold m-0">Announcements</h1>
      <button
        on:click={handleSync}
        disabled={syncing}
        class="px-3 py-1.5 rounded-lg font-medium text-sm transition bg-discord-blurple hover:bg-blue-600 text-white disabled:opacity-50"
      >
        {syncing ? 'Syncing…' : 'Sync now'}
      </button>
    </div>
    <p class="text-sm text-gray-500 m-0">
      A mirror of the linked Discord channel's messages. Manage which channel this pulls from on the <a
        href="/server"
        class="text-discord-blurple hover:underline">Discord server</a
      > page.
    </p>

    {#if error}
      <p class="text-sm text-red-600" role="alert">{error}</p>
    {/if}

    {#if adoptedMessage}
      <p class="text-sm text-green-700 bg-green-100 rounded-lg px-3 py-2" role="status">
        {adoptedMessage}
      </p>
    {/if}

    {#if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if posts.length > 0}
      {#each posts as post, i (post.id)}
        <!-- Only the first pinned post is featured. The list already comes
             back pinned-first from the backend, so this is the top one when
             any are pinned. -->
        <AnnouncementPostCard
          {post}
          index={i}
          featured={post.pinned && i === 0}
          onAdopt={(p) => {
            adoptedMessage = '';
            adopting = p;
          }}
        />
      {/each}
    {:else if !error}
      <p class="text-sm text-gray-500">
        Nothing synced yet. Click "Sync now" to pull in the channel's messages.
      </p>
    {/if}
  </div>

  {#if adopting}
    <AdoptEventModal
      post={adopting}
      on:close={() => (adopting = null)}
      on:adopted={(e) => handleAdopted(e.detail.rsvps)}
    />
  {/if}
</Frame>

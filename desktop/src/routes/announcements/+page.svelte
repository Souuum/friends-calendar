<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import AnnouncementPostCard from '$lib/components/molecules/AnnouncementPostCard.svelte';
  import type { AnnouncementPostInfo } from '$lib/types';

  let posts: AnnouncementPostInfo[] = [];
  let loading = true;
  let error = '';
  let syncing = false;

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
  <div class="max-w-2xl mx-auto py-6 px-3 sm:px-4 space-y-4 anim-fade-up">
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
      A mirror of the linked Discord channel's messages. Manage which channel this pulls from on
      the <a href="/server" class="text-discord-blurple hover:underline">Discord server</a> page.
    </p>

    {#if error}
      <p class="text-sm text-red-600" role="alert">{error}</p>
    {/if}

    {#if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if posts.length > 0}
      {#each posts as post, i (post.id)}
        <!-- Only the first pinned post is featured. The list already comes
             back pinned-first from the backend, so this is the top one when
             any are pinned. -->
        <AnnouncementPostCard {post} index={i} featured={post.pinned && i === 0} />
      {/each}
    {:else if !error}
      <p class="text-sm text-gray-500">
        Nothing synced yet. Click "Sync now" to pull in the channel's messages.
      </p>
    {/if}
  </div>
</Frame>

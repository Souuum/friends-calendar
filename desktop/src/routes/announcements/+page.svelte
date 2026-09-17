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

  // Composing, per both mockups ("New announcement" on desktop, a pill above
  // the tab bar on mobile). Posts through a webhook so it carries your name
  // and cannot ping the server with the bot's permissions - see
  // services::discord_webhook.
  let composing = false;
  let draft = '';
  let posting = false;
  let composeError = '';

  async function post() {
    const content = draft.trim();
    if (!content) return;
    try {
      posting = true;
      composeError = '';
      // The response is the refreshed feed, so the poster sees their own
      // message without hitting "Sync now".
      posts = await api.composeAnnouncement(content);
      draft = '';
      composing = false;
    } catch (err) {
      // Draft kept: retyping is the annoying half of a failed post.
      composeError = err instanceof Error ? err.message : 'Could not post';
    } finally {
      posting = false;
    }
  }

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
      <div class="flex gap-2">
        <button
          on:click={handleSync}
          disabled={syncing}
          class="rounded-[9px] border border-line bg-surface px-3.5 py-[9px] text-[13px] font-semibold hover:bg-subtle disabled:opacity-50"
        >
          {syncing ? 'Syncing…' : 'Sync now'}
        </button>
        <button
          on:click={() => (composing = !composing)}
          class="rounded-[9px] bg-primary px-3.5 py-[9px] text-[13px] font-semibold text-white hover:bg-primary-active"
        >
          New announcement
        </button>
      </div>
    </div>
    <p class="text-sm text-gray-500 m-0">
      A mirror of the linked Discord channel's messages. Manage which channel this pulls from on the <a
        href="/server"
        class="text-discord-blurple hover:underline">Discord server</a
      > page.
    </p>

    {#if composing}
      <form
        on:submit|preventDefault={post}
        class="flex flex-col gap-2 rounded-[14px] border border-line bg-surface p-[18px]"
      >
        <label for="announcement" class="text-[13px] font-medium text-body">
          Post to the channel as you
        </label>
        <textarea
          id="announcement"
          bind:value={draft}
          rows="3"
          placeholder="What's happening?"
          class="w-full resize-y rounded-[11px] border border-line bg-surface px-3 py-2.5 text-[14px] outline-none focus:border-primary"
        ></textarea>
        {#if composeError}
          <p class="m-0 text-[13px] text-red-600" role="alert">{composeError}</p>
        {/if}
        <p class="m-0 text-[12px] text-muted">
          Posted under your name. Mentions won't ping anyone — the app can't ping the server on your
          behalf.
        </p>
        <div class="flex gap-2">
          <button
            type="submit"
            disabled={posting || draft.trim() === ''}
            class="rounded-[9px] bg-primary px-3.5 py-[9px] text-[13px] font-semibold text-white hover:bg-primary-active disabled:opacity-50"
          >
            {posting ? 'Posting…' : 'Post'}
          </button>
          <button
            type="button"
            on:click={() => (composing = false)}
            class="rounded-[9px] border border-line bg-surface px-3.5 py-[9px] text-[13px] font-semibold hover:bg-subtle"
          >
            Cancel
          </button>
        </div>
      </form>
    {/if}

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

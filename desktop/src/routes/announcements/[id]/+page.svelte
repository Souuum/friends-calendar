<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import AnnouncementPostCard from '$lib/components/molecules/AnnouncementPostCard.svelte';
  import { formatDate } from '$lib/utils/dateUtils';
  import type { AnnouncementPostInfo, ReplyInfo } from '$lib/types';

  // Built mobile-first per mockup screen 08: one column, composer docked to
  // the bottom below md:, inline above it.
  let post: AnnouncementPostInfo | undefined;
  let replies: ReplyInfo[] = [];
  let loading = true;
  let error = '';

  let draft = '';
  let sending = false;
  let sendError = '';

  $: postId = $page.params.id;

  async function load(id: string) {
    try {
      loading = true;
      error = '';
      // No GET /api/announcements/:id - the list is already cached locally
      // and small (one guild, one channel), so finding the post in it beats
      // adding an endpoint for a single row.
      const [posts, fetched] = await Promise.all([
        api.getAnnouncements(),
        api.getAnnouncementReplies(id)
      ]);
      post = posts.find((p) => p.id === id);
      replies = fetched;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load the thread';
    } finally {
      loading = false;
    }
  }

  async function send() {
    const body = draft.trim();
    // `$page.params.id` is typed optional; the composer is only reachable
    // once the post loaded, but the guard keeps that a fact rather than an
    // assumption.
    if (!body || !postId) return;

    try {
      sending = true;
      sendError = '';
      // The endpoint returns the refreshed thread, so there's no second
      // round-trip and no optimistic guess at what Discord will store.
      replies = await api.postAnnouncementReply(postId, body);
      draft = '';
    } catch (err) {
      sendError = err instanceof Error ? err.message : 'Failed to post your reply';
    } finally {
      sending = false;
    }
  }

  onMount(() => {
    if (postId) load(postId);
  });
</script>

<svelte:head>
  <title>Thread - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl mx-auto py-6 px-3 sm:px-4 anim-fade-up">
    <button
      class="text-sm text-discord-blurple hover:underline mb-4 bg-transparent border-none cursor-pointer p-0"
      on:click={() => goto('/announcements')}
    >
      ‹ Hub
    </button>

    {#if error}
      <p class="text-sm text-red-600" role="alert">{error}</p>
    {:else if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if !post}
      <p class="text-sm text-gray-500">That announcement isn't in the synced feed.</p>
    {:else}
      <AnnouncementPostCard {post} linkToThread={false} />

      <h2 class="font-mono text-[10px] tracking-widest uppercase text-muted mt-6 mb-2">
        {replies.length}
        {replies.length === 1 ? 'reply' : 'replies'}
      </h2>

      {#if replies.length === 0}
        <p class="text-sm text-gray-500">No replies yet — say something.</p>
      {:else}
        <div class="flex flex-col gap-3">
          {#each replies as reply, i (`${reply.posted_at}-${i}`)}
            <div class="flex gap-3 bg-surface border border-line rounded-xl p-3.5">
              {#if reply.author_avatar_url}
                <img src={reply.author_avatar_url} alt="" class="w-8 h-8 rounded-full shrink-0" />
              {:else}
                <div class="w-8 h-8 rounded-full bg-gray-300 shrink-0"></div>
              {/if}
              <div class="min-w-0 flex-1">
                <div class="flex items-baseline gap-2">
                  <span class="text-sm font-semibold text-gray-900">{reply.author_username}</span>
                  <span class="font-mono text-[10px] text-muted">{formatDate(reply.posted_at)}</span
                  >
                </div>
                <p class="text-sm text-body whitespace-pre-wrap m-0 mt-1">{reply.body}</p>
              </div>
            </div>
          {/each}
        </div>
      {/if}

      {#if sendError}
        <p class="text-sm text-red-600 mt-3" role="alert">{sendError}</p>
      {/if}

      <!-- Docked above the bottom tab bar on mobile, inline on desktop. The
           bottom padding on the list side is what stops the last reply
           hiding behind it. -->
      <form
        on:submit|preventDefault={send}
        class="flex gap-2 mt-4
               fixed inset-x-0 bottom-[68px] z-30 bg-surface border-t border-line px-3 py-3
               md:static md:z-auto md:border-0 md:bg-transparent md:px-0 md:py-0"
      >
        <input
          type="text"
          bind:value={draft}
          placeholder="Reply in Discord…"
          aria-label="Reply"
          class="flex-1 min-w-0 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
        />
        <button
          type="submit"
          disabled={sending || !draft.trim()}
          class="px-4 py-2 bg-primary text-white rounded-lg text-sm font-semibold disabled:opacity-50"
        >
          {sending ? 'Sending…' : 'Send'}
        </button>
      </form>
      <!-- Space for the docked composer so it never covers the last reply. -->
      <div class="h-24 md:hidden"></div>
    {/if}
  </div>
</Frame>

<script lang="ts">
  import Icon from '$lib/components/atoms/Icon.svelte';
  import { renderDiscordMarkdown } from '$lib/utils/discordMarkdown';
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

  let draft = '';
  let sending = false;
  let replyError = '';

  async function sendReply() {
    const content = draft.trim();
    // postId comes from a route param, so TypeScript has it as possibly
    // undefined; there is no thread to reply in without one.
    if (!content || !postId) return;
    try {
      sending = true;
      replyError = '';
      // The endpoint returns the refreshed thread, so there is no second
      // round-trip and no optimistic guess to reconcile.
      replies = await api.postAnnouncementReply(postId, content);
      draft = '';
    } catch (err) {
      // The draft is deliberately kept - retyping a lost reply is the
      // annoying half of a failed send.
      replyError = err instanceof Error ? err.message : 'Could not post your reply';
    } finally {
      sending = false;
    }
  }
  let loading = true;
  let error = '';

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

  onMount(() => {
    if (postId) load(postId);
  });
</script>

<svelte:head>
  <title>Thread - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl anim-fade-up">
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
                <p class="text-sm text-body m-0 mt-1">
                  {@html renderDiscordMarkdown(reply.body)}
                </p>
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Replying was removed on 2026-09-17 because the *bot* sent it: the
           thread showed "friends-calendar" saying whatever a user typed, and
           with no allowed_mentions guard a user could make the bot ping
           @everyone with the bot's permissions. It is back on a webhook,
           which carries the author's name and face and suppresses mentions -
           see services::discord_webhook. -->
      <form on:submit|preventDefault={sendReply} class="mt-4 flex flex-col gap-2">
        <label for="reply" class="sr-only">Reply</label>
        <textarea
          id="reply"
          bind:value={draft}
          rows="2"
          placeholder="Reply in the thread…"
          class="w-full resize-y rounded-[11px] border border-line bg-surface px-3 py-2.5 text-[14px] outline-none focus:border-primary"
        ></textarea>
        {#if replyError}
          <p class="m-0 text-[13px] text-red-600" role="alert">{replyError}</p>
        {/if}
        <div class="flex items-center gap-3">
          <button
            type="submit"
            disabled={sending || draft.trim() === ''}
            class="rounded-[9px] bg-primary px-3.5 py-[9px] text-[13px] font-semibold text-white hover:bg-primary-active disabled:opacity-50"
          >
            {sending ? 'Posting…' : 'Reply'}
          </button>
          {#if post?.thread_url}
            <a
              href={post.thread_url}
              target="_blank"
              rel="noopener noreferrer"
              class="text-[13px] font-semibold text-primary no-underline hover:underline"
            >
              Open in Discord
            </a>
          {/if}
        </div>
      </form>
    {/if}
  </div>
</Frame>

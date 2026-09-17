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

      <!-- Replies used to be posted from here, but the *bot* sent them: the
           thread showed "friends-calendar" saying whatever a user typed, with
           no attribution, and with no allowed_mentions guard a user could
           make the bot ping @everyone using the bot's permissions rather
           than their own. Reading stays; writing goes to Discord, where the
           message is actually attributed to the person who wrote it. -->
      {#if post?.thread_url}
        <a
          href={post.thread_url}
          target="_blank"
          rel="noopener noreferrer"
          class="inline-flex items-center gap-2 mt-4 px-4 py-2 bg-primary text-white rounded-lg text-sm font-semibold no-underline"
        >
          <Icon name="replies" size={16} />
          Reply in Discord
        </a>
      {:else}
        <p class="text-sm text-muted mt-4">
          Open this thread in Discord to reply — no server is linked yet, so there's no link to give
          you.
        </p>
      {/if}
    {/if}
  </div>
</Frame>

<script lang="ts">
  import type { AnnouncementPostInfo } from '$lib/types';
  import Avatar from '$lib/components/atoms/Avatar.svelte';
  import { formatDate } from '$lib/utils/dateUtils';

  // Presentational mirror of a synced Discord message - see
  // services::discord_feed on the backend. Read-only: this is a mirror of
  // what's already in Discord, not a second place to post from.
  export let post: AnnouncementPostInfo;

  function tagLabel(tag: string): string {
    return tag === 'event' ? '📅 Event' : '💬 General';
  }

  function tagColor(tag: string): string {
    return tag === 'event' ? 'bg-blue-100 text-blue-800' : 'bg-gray-100 text-gray-700';
  }
</script>

<div class="bg-white rounded-lg shadow p-5 space-y-3">
  <div class="flex justify-between items-start gap-3">
    <div class="flex items-center gap-2 min-w-0">
      <Avatar src={post.author_avatar_url ?? ''} size={32} />
      <div class="min-w-0">
        <p class="font-semibold text-gray-900 truncate m-0">{post.author_username}</p>
        <p class="text-xs text-gray-500 m-0">{formatDate(post.posted_at)}</p>
      </div>
    </div>
    <div class="flex items-center gap-2 shrink-0">
      {#if post.pinned}
        <span class="text-xs px-2 py-1 rounded bg-yellow-100 text-yellow-800">📌 Pinned</span>
      {/if}
      <span class="text-xs px-2 py-1 rounded {tagColor(post.tag)}">{tagLabel(post.tag)}</span>
    </div>
  </div>

  {#if post.title}
    <h3 class="font-bold text-lg text-gray-900 m-0">{post.title}</h3>
  {/if}

  <p class="text-gray-700 text-sm whitespace-pre-wrap m-0">{post.body}</p>

  <div class="flex items-center gap-4 text-sm text-gray-500 border-t border-gray-100 pt-2">
    <span>👍 {post.reaction_count}</span>
    <span>💬 {post.reply_count} {post.reply_count === 1 ? 'reply' : 'replies'}</span>
  </div>
</div>

<script lang="ts">
  import Icon from '$lib/components/atoms/Icon.svelte';
  import type { AnnouncementPostInfo } from '$lib/types';
  import Avatar from '$lib/components/atoms/Avatar.svelte';
  import { formatDate } from '$lib/utils/dateUtils';

  // Presentational mirror of a synced Discord message - see
  // services::discord_feed on the backend. Read-only: this is a mirror of
  // what's already in Discord, not a second place to post from.
  export let post: AnnouncementPostInfo;
  // Position in the feed, purely for the staggered entrance delay - mirrors
  // the mockup's `posts` list, where each card's fade-up animation-delay is
  // offset from the one before it rather than firing all at once.
  export let index = 0;
  /**
   * Inverted treatment for the pinned post, per the mockup's Hub screen.
   * A flag rather than reading `post.pinned` directly: only the *first*
   * pinned post gets it - several inverted cards in a row would just be a
   * dark feed, which defeats the point of singling one out.
   */
  export let featured = false;
  /**
   * Off on the thread page itself, where the card is the thread's own
   * header - linking from there back to where you already are is noise.
   */
  export let linkToThread = true;

  // Icon and text are separate now: an emoji glued into the string
  // couldn't be styled, sized, or follow the theme.
  function tagIcon(tag: string): 'calendar' | 'replies' {
    return tag === 'event' ? 'calendar' : 'replies';
  }

  function tagLabel(tag: string): string {
    return tag === 'event' ? 'Event' : 'General';
  }

  function tagColor(tag: string): string {
    return tag === 'event' ? 'bg-blue-100 text-blue-800' : 'bg-gray-100 text-gray-700';
  }
</script>

<div
  class="rounded-lg shadow p-5 space-y-3 anim-fade-up-stagger {featured
    ? 'bg-[#171719] text-white'
    : 'bg-white'}"
  style="animation-delay: {index * 60}ms"
>
  <div class="flex justify-between items-start gap-3">
    <div class="flex items-center gap-2 min-w-0">
      <Avatar src={post.author_avatar_url ?? ''} size={32} />
      <div class="min-w-0">
        <p class="font-semibold truncate m-0 {featured ? 'text-white' : 'text-gray-900'}">
          {post.author_username}
        </p>
        <p class="text-xs m-0 {featured ? 'text-muted' : 'text-gray-500'}">
          {formatDate(post.posted_at)}
        </p>
      </div>
    </div>
    <div class="flex items-center gap-2 shrink-0">
      {#if post.pinned}
        <span
          class="inline-flex items-center gap-1 text-xs px-2 py-1 rounded bg-yellow-100 text-yellow-800"
        >
          <Icon name="pinned" size={12} /> Pinned
        </span>
      {/if}
      <span class="inline-flex items-center gap-1 text-xs px-2 py-1 rounded {tagColor(post.tag)}">
        <Icon name={tagIcon(post.tag)} size={12} />
        {tagLabel(post.tag)}
      </span>
    </div>
  </div>

  {#if post.title}
    <h3 class="font-bold text-lg m-0 {featured ? 'text-white' : 'text-gray-900'}">{post.title}</h3>
  {/if}

  <p class="text-sm whitespace-pre-wrap m-0 {featured ? 'text-[#ebebeb]' : 'text-gray-700'}">
    {post.body}
  </p>

  <div
    class="flex items-center gap-4 text-sm border-t pt-2 {featured
      ? 'text-muted border-white/10'
      : 'text-gray-500 border-gray-100'}"
  >
    <span class="inline-flex items-center gap-1">
      <Icon name="reactions" size={14} />
      {post.reaction_count}
    </span>
    <span class="inline-flex items-center gap-1">
      <Icon name="replies" size={14} />
      {post.reply_count}
      {post.reply_count === 1 ? 'reply' : 'replies'}
    </span>
    {#if linkToThread}
      <a
        href={`/announcements/${post.id}`}
        class="ml-auto text-xs font-semibold {featured ? 'text-white' : 'text-discord-blurple'}"
      >
        Open thread
      </a>
    {/if}
  </div>
</div>

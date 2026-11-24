<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import AnnouncementPostCard from '$lib/components/molecules/AnnouncementPostCard.svelte';
  import { ANNOUNCEMENT_BOT, ANNOUNCEMENT_HANDWRITTEN } from '../../fixtures';

  /**
   * One Discord message from the announcements channel, mirrored as a card:
   * author, date, tags, optional title, the body with Discord markdown rendered,
   * and reaction and reply counts. Read-only. Use one per post in a feed.
   *
   * ```svelte
   * {#each posts as post, i (post.id)}
   *   <AnnouncementPostCard
   *     {post}
   *     index={i}
   *     featured={post.pinned && i === 0}
   *     onAdopt={(p) => (adopting = p)}
   *   />
   * {/each}
   * ```
   *
   * - "Add to calendar" shows only when `onAdopt` is set and the post is not
   *   tagged `event`. Clicking it calls `onAdopt(post)`.
   * - "Open thread" is a link to `/announcements/{post.id}`, shown while
   *   `linkToThread` is on.
   * - The card fades up on mount, delayed by `index * 60` ms.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/AnnouncementPostCard',
    component: AnnouncementPostCard,
    args: { post: ANNOUNCEMENT_BOT }
  });
</script>

<!-- A pinned post tagged `event`, with a title and a body in Discord markdown. -->
<Story name="Event announcement" />

<!-- `featured` inverts the card, as the feed does for its first pinned post. -->
<Story name="Featured (first pinned)" args={{ featured: true }} />

<!-- A `general` post with `onAdopt` set: the "Add to calendar" button appears. -->
<Story name="Hand-written, adoptable" args={{ post: ANNOUNCEMENT_HANDWRITTEN, onAdopt: fn() }} />

<!-- A post with no `thread_url`: the card does not read that field, so "Open thread" is still shown. -->
<Story
  name="No thread link"
  args={{ post: { ...ANNOUNCEMENT_HANDWRITTEN, thread_url: undefined } }}
/>

<!-- `linkToThread={false}` drops "Open thread", as on the thread page where the card is the header. -->
<Story name="On the thread page" args={{ linkToThread: false }} />

<!-- `index` only delays the entrance: at 8 the card fades up 480 ms after mount. -->
<Story name="Later in the feed" args={{ post: ANNOUNCEMENT_HANDWRITTEN, index: 8 }} />

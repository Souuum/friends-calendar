<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import AdoptEventModal from '$lib/components/organisms/AdoptEventModal.svelte';
  import { user } from '$lib/stores';
  import { ANNOUNCEMENT_BOT, ANNOUNCEMENT_HANDWRITTEN, EVENT_HOSTED, ME } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  /**
   * The "Add to calendar" dialog: turns an announcement that is already in
   * Discord into a calendar event. It prefills a form from the post's text and
   * lets the user correct it before saving. Mount it only while a post is being
   * adopted.
   *
   * ```svelte
   * {#if adopting}
   *   <AdoptEventModal
   *     post={adopting}
   *     on:close={() => (adopting = null)}
   *     on:adopted={(e) => handleAdopted(e.detail.rsvps)}
   *   />
   * {/if}
   * ```
   *
   * - Dispatches `close` (the × button, Cancel, Escape, a backdrop tap or the
   *   back gesture) and `adopted` with `{ rsvps }` once
   *   `api.adoptAnnouncement()` succeeds. It never unmounts itself.
   * - Renders a `position: fixed` full-screen overlay.
   * - Reads the `user` store for the default visibility (`friends` if unset).
   * - The form is `novalidate` and checks itself: a missing title, start or end
   *   time, an end before the start, or a failed request shows inline.
   */
  const { Story } = defineMeta({
    title: 'Components/Organisms/AdoptEventModal',
    component: AdoptEventModal,
    parameters: {
      layout: 'fullscreen',
      docs: { story: { inline: false, height: '760px' } }
    },
    args: { post: ANNOUNCEMENT_BOT },
    beforeEach: () => {
      user.set(ME);
      const restore = mockApi({
        adoptAnnouncement: () =>
          delay({ event: EVENT_HOSTED, rsvps_recorded: 4, backfill_failed: false })
      });
      return () => {
        restore();
        user.set(null);
      };
    }
  });
</script>

<!-- The bot's own template parses cleanly, so the form arrives prefilled. -->
<Story name="Parsed announcement" />

<!-- No <t:…> timestamp: the form names what it couldn't read. -->
<Story name="Unparseable post" args={{ post: ANNOUNCEMENT_HANDWRITTEN }} />

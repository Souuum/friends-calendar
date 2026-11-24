<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CalendarView from '$lib/components/CalendarView.svelte';
  import { user } from '$lib/stores';
  import type { EventWithParticipants } from '$lib/types';
  import { BEST_SLOT, BUSY, EVENTS, FRIENDS, GUILDS, INVITE_URL, ME } from '../../fixtures';
  import { mockApi, pending } from '../../mockApi';

  /** Everything the root screen and the calendar inside it fetch. */
  function withEvents(getEvents: () => Promise<EventWithParticipants[]>) {
    return () => {
      user.set(ME);
      const restore = mockApi({
        getEvents,
        getFreeFriendsNow: async () => ['u1', 'u2'],
        getFriends: async () => FRIENDS,
        getExternalBusy: async () => BUSY,
        getBestSlots: async () => [BEST_SLOT],
        getServers: async () => ({ guilds: GUILDS, invite_url: INVITE_URL })
      });
      return () => {
        restore();
        user.set(null);
      };
    };
  }

  /**
   * The signed-in screen of the root route: fetches the caller's events, then hands them to
   * `Calendar`. It is the only place `Calendar` is mounted, at `/` inside `Frame`.
   *
   * ```svelte
   * <Frame>
   *   <CalendarView />
   * </Frame>
   * ```
   *
   * - No props, no slot, no events.
   * - On mount it calls `api.getEvents()` and sorts the result by start time. It calls it again
   *   each time `Calendar` dispatches `refresh` (after a save, a delete or an RSVP).
   * - While the first load is pending it shows a spinner; if the load fails it shows the error
   *   in place of the calendar.
   * - With no events the calendar still renders, under a one-line hint, so "+ New Event" stays
   *   reachable.
   * - Everything `Calendar` fetches happens here too - see its page for the list.
   */
  const { Story } = defineMeta({
    title: 'Components/CalendarView',
    component: CalendarView,
    parameters: {
      layout: 'fullscreen',
      // Every story stubs `api` and sets the `user` store, and the create form inside the
      // calendar is a fixed overlay, so on the docs page each story needs a frame of its own.
      docs: { story: { inline: false, height: '760px' } }
    }
  });
</script>

<!-- `api.getEvents()` resolved with events: the full calendar. -->
<Story name="Loaded" beforeEach={withEvents(async () => EVENTS)} />

<!-- No events: the calendar still renders, so "+ New Event" stays reachable. -->
<Story name="No events yet" beforeEach={withEvents(async () => [])} />

<!-- `api.getEvents()` still pending: the spinner and "Loading events...". -->
<Story name="Loading" beforeEach={withEvents(() => pending())} />

<!-- `api.getEvents()` rejected: the error message replaces the calendar. -->
<Story
  name="Failed to load"
  beforeEach={withEvents(async () => {
    throw new Error('Could not reach the server.');
  })}
/>

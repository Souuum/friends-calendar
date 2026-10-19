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

  // The root route's signed-in screen: fetches the events, then renders Calendar.
  const { Story } = defineMeta({
    title: 'Components/CalendarView',
    component: CalendarView,
    parameters: { layout: 'fullscreen' }
  });
</script>

<Story name="Loaded" beforeEach={withEvents(async () => EVENTS)} />
<!-- No events: the calendar still renders, so "+ New Event" stays reachable. -->
<Story name="No events yet" beforeEach={withEvents(async () => [])} />
<Story name="Loading" beforeEach={withEvents(() => pending())} />
<Story
  name="Failed to load"
  beforeEach={withEvents(async () => {
    throw new Error('Could not reach the server.');
  })}
/>

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Calendar from '$lib/components/templates/Calendar.svelte';
  import { user } from '$lib/stores';
  import { BEST_SLOT, BUSY, EVENTS, FRIENDS, GUILDS, INVITE_URL, ME } from '../../fixtures';
  import { mockApi } from '../../mockApi';

  /**
   * The whole calendar screen for a list of events the parent has already fetched: the header
   * (previous / next / today, Month / Week / Day / List, "+ New Event"), the "Free tonight" bar,
   * the filter chips, the grid for the chosen view, and the event detail panel. `CalendarView`
   * is the one place that mounts it.
   *
   * ```svelte
   * <Calendar {events} on:refresh={loadEvents} />
   * ```
   *
   * - Dispatches `refresh` after an event is saved, deleted or answered. It never refetches
   *   `events` itself - the parent reloads and passes the new list back in.
   * - Owns the create / edit form (`CreateEventModal`) and the detail panel (`EventPeekPanel`),
   *   so the parent renders neither.
   * - Fetches on mount: `api.getFreeFriendsNow()` and `api.getFriends()` for the "Free tonight"
   *   bar (a failure is shown inside the bar), `api.getBestSlots()` for "Best overlap this
   *   week", and `api.getExternalBusy()` for the anonymous busy bands - again whenever the
   *   visible range changes. The last two fail silently.
   * - The filter chips work on the `events` already passed in; they make no request.
   * - No slot.
   */
  const { Story } = defineMeta({
    title: 'Components/Templates/Calendar',
    component: Calendar,
    parameters: {
      layout: 'fullscreen',
      // Every story stubs `api` and sets the `user` store, and the create form is a fixed
      // overlay, so on the docs page each story needs a frame of its own.
      docs: { story: { inline: false, height: '760px' } }
    },
    beforeEach: () => {
      user.set(ME);
      const restore = mockApi({
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
    }
  });
</script>

<!-- `events` collides with a Svelte mount option, so it goes through a template. -->
{#snippet withEvents()}
  <Calendar events={EVENTS} />
{/snippet}

<!-- `events` filled: chips in the grid, two friends free tonight, a best-overlap suggestion and imported busy bands. -->
<Story name="With events" template={withEvents} />

<!-- `events` left at its default `[]`: the calendar still renders, with "+ New Event" reachable. -->
<Story name="Empty" />

<!-- Nobody is free and no slot is suggested: the bar says "No friends free right now". -->
<Story
  name="No friends free"
  beforeEach={() => mockApi({ getFreeFriendsNow: async () => [], getBestSlots: async () => [] })}
  template={withEvents}
/>

<!-- `api.getFreeFriendsNow()` rejects: the error shows inside the bar and the calendar still works. -->
<Story
  name="Availability failed to load"
  beforeEach={() =>
    mockApi({
      getFreeFriendsNow: async () => {
        throw new Error('Could not reach the server.');
      }
    })}
  template={withEvents}
/>

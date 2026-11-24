<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventCardImpl from '$lib/components/EventCardImpl.svelte';
  import { EVENT_DISCOVERED, EVENT_HOSTED, EVENT_INVITED, EVENT_MAYBE } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  /**
   * The event card with the API attached: wraps the presentational `EventCard` atom and owns
   * the calls for answering an invitation (`api.updateParticipation()`) and deleting an event
   * (`api.deleteEvent()`). `EventTooltip` is the one place that mounts it, for the month
   * view's hover card.
   *
   * ```svelte
   * <EventCardImpl {event} on:refresh={reloadEvents} />
   * ```
   *
   * - Dispatches `refresh`, with no payload, after one of those calls succeeds - the parent
   *   reloads the events. A failure goes to `alert()`, and deleting asks through `confirm()`.
   * - What the card shows follows `event`: the "Creator" badge and "Delete Event" when
   *   `is_creator`; the Accept / Maybe / Decline row when you are not the creator and have a
   *   `my_status`; neither for an event you can see but were not invited to.
   * - As wired today the handlers are not reached: it listens for `delete`, `accepted`,
   *   `maybe` and `declined`, while `EventCard` dispatches `deleted` and does not forward the
   *   status row's events. So an RSVP button only moves the highlight locally, "Delete Event"
   *   does nothing, and `refresh` is not dispatched.
   * - No slot. Reads no store and fetches nothing on mount.
   */
  const { Story } = defineMeta({
    title: 'Components/EventCardImpl',
    component: EventCardImpl,
    args: { event: EVENT_HOSTED },
    parameters: {
      // Every story stubs `api`, so on the docs page each story needs a frame of its own.
      docs: { story: { inline: false, height: '420px' } }
    },
    beforeEach: () =>
      mockApi({ updateParticipation: () => delay(undefined), deleteEvent: () => delay(undefined) })
  });
</script>

<!-- `event.is_creator` true: the "Creator" badge and the "Delete Event" button, no RSVP row. -->
<Story name="Hosted by me" />

<!-- An invitation still `pending`: the RSVP row with no button filled. -->
<Story name="Invited" args={{ event: EVENT_INVITED }} />

<!-- `event.my_status` is `maybe`: the RSVP row with "Maybe" filled. -->
<Story name="Answered maybe" args={{ event: EVENT_MAYBE }} />

<!-- A public event you were not invited to (no `my_status`): no RSVP row and no delete button. -->
<Story name="Not invited" args={{ event: EVENT_DISCOVERED }} />

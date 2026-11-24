<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventPeekPanel from '$lib/components/organisms/EventPeekPanel.svelte';
  import {
    EVENT_DECLINED,
    EVENT_DISCOVERED,
    EVENT_HOSTED,
    EVENT_INVITED,
    EVENT_MAYBE
  } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  /**
   * The detail surface for the event selected on the calendar: time, place,
   * guests, and the actions that fit the viewer - RSVP for a guest, edit, nudge
   * and delete for the organiser.
   *
   * ```svelte
   * <EventPeekPanel
   *   event={selectedEvent}
   *   on:close={() => (selectedEvent = null)}
   *   on:refresh={handleRefresh}
   *   on:edit={(e) => openEditModal(e.detail)}
   *   on:deleted={handleDeleted}
   * />
   * ```
   *
   * - One component, two placements: a static 296px column from `lg:` up, and a
   *   `position: fixed` bottom sheet with a backdrop below it. A docs frame is
   *   narrower than `lg:`, so the stories here show the sheet.
   * - Dispatches `close` (sheet only: backdrop, × button, Escape, back
   *   gesture), `refresh` after an RSVP is saved, `edit` with the event as
   *   detail, and `deleted` after the event is deleted.
   * - Calls `api.updateParticipation()`, `api.deleteEvent()` and
   *   `api.nudgeNoAnswers()` itself; a failure shows inline in the panel.
   */
  const { Story } = defineMeta({
    title: 'Components/Organisms/EventPeekPanel',
    component: EventPeekPanel,
    parameters: {
      layout: 'fullscreen',
      docs: { story: { inline: false, height: '680px' } }
    },
    args: { event: EVENT_HOSTED },
    beforeEach: () =>
      mockApi({
        updateParticipation: () => delay(undefined),
        deleteEvent: () => delay(undefined),
        nudgeNoAnswers: () => delay({ nudged: 1, dms_sent: 1, discord_failed: false })
      })
  });
</script>

<!-- An `event` the viewer created: Edit, Nudge no-answers and Delete event instead of RSVP buttons. -->
<Story name="Hosted by me" />

<!-- An `event` the viewer is invited to and has not answered: Going / Maybe / Can't, none chosen. -->
<Story name="Invited, awaiting my answer" args={{ event: EVENT_INVITED }} />

<!-- An `event` the viewer answered "maybe" to: the RSVP row stays, so the answer can change. -->
<Story name="Answered maybe" args={{ event: EVENT_MAYBE }} />

<!-- An `event` the viewer declined. -->
<Story name="Declined" args={{ event: EVENT_DECLINED }} />

<!-- A public `event` the viewer is not a participant of: a note on why it is visible, and no RSVP row. -->
<Story name="Discovered (public, not invited)" args={{ event: EVENT_DISCOVERED }} />

<!-- `event={null}`: from `lg:` up the column shows a hint; below that nothing renders. -->
<Story name="Nothing selected" args={{ event: null }} />

<!-- The nudge request fails: press "Nudge no-answers" to see the error inline. -->
<Story
  name="Nudge rate-limited"
  beforeEach={() =>
    mockApi({
      nudgeNoAnswers: () =>
        delay(null, 300).then(() => {
          throw new Error('You can nudge again in 23 hours.');
        })
    })}
/>

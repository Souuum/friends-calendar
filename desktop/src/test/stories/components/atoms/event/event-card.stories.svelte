<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventCard from '$lib/components/atoms/event/EventCard.svelte';
  import {
    EVENT_CREATOR,
    EVENT_PARTICIPANT_ACCEPTED,
    EVENT_PARTICIPANTS_5,
    EVENT_PARTICIPANTS_6
  } from './mock';

  const EVENT_PARTICIPANT_PENDING = { ...EVENT_PARTICIPANT_ACCEPTED, my_status: 'pending' };

  /**
   * The full card for one event: title, description, date, location, the
   * guest list, and either the answer buttons or, for the creator, a delete
   * button.
   *
   * ```svelte
   * <EventCard {event} on:deleted={(e) => removeEvent(e.detail)} />
   * ```
   *
   * The creator's "Delete Event" button dispatches `deleted` with the event as
   * its detail; the card deletes nothing itself. The answer buttons only
   * change `event.my_status` on the object the card was given: the card does
   * not forward their events and saves nothing. The guest list is an
   * `EventCardParticipant` and the buttons an `EventCardStatusBar`.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/EventCard',
    component: EventCard
  });
</script>

<!-- `is_creator: true`: a "Creator" badge and the delete button, no answer buttons. -->
<Story name="Default Creator" args={{ event: EVENT_CREATOR }} />

<!-- An invited `event` with `my_status: 'accepted'`: the answer buttons, with Accept filled. -->
<Story name="Default Participant" args={{ event: EVENT_PARTICIPANT_ACCEPTED }} />

<!-- `my_status: 'pending'`: the answer buttons with none of them filled. -->
<Story name="Awaiting answer" args={{ event: EVENT_PARTICIPANT_PENDING }} />

<!-- Five participants: the most the guest list shows by name. -->
<Story name="Default Participant 5" args={{ event: EVENT_PARTICIPANTS_5 }} />

<!-- Six participants: the sixth is folded into "+1 more". -->
<Story name="Default participant 6" args={{ event: EVENT_PARTICIPANTS_6 }} />

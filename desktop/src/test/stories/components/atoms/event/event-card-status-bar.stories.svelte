<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventCardStatusBar from '$lib/components/atoms/event/EventCardStatusBar.svelte';

  /**
   * The three answer buttons of an event card: Accept, Maybe and Decline,
   * with the current answer filled in.
   *
   * ```svelte
   * <EventCardStatusBar
   *   bind:status={event.my_status}
   *   on:accepted={() => save('accepted')}
   *   on:maybe={() => save('maybe')}
   *   on:declined={() => save('declined')}
   * />
   * ```
   *
   * Clicking a button sets `status` (bind it to keep the value) and dispatches
   * an event named after the answer: `accepted`, `maybe` or `declined`. The
   * events carry no detail.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/EventCardStatusBar',
    component: EventCardStatusBar,
    argTypes: {
      status: { control: 'inline-radio', options: ['pending', 'accepted', 'maybe', 'declined'] }
    },
    args: { status: 'pending' }
  });
</script>

<!-- `status="pending"`: no button is filled. -->
<Story name="Pending" />

<!-- `status="accepted"` fills Accept in green. -->
<Story name="Accepted" args={{ status: 'accepted' }} />

<!-- `status="maybe"` fills Maybe in yellow. -->
<Story name="Maybe" args={{ status: 'maybe' }} />

<!-- `status="declined"` fills Decline in red. -->
<Story name="Declined" args={{ status: 'declined' }} />

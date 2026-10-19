<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventPeekPanel from '$lib/components/organisms/EventPeekPanel.svelte';
  import { EVENT_HOSTED, EVENT_INVITED, EVENT_DISCOVERED } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  const { Story } = defineMeta({
    title: 'Components/Organisms/EventPeekPanel',
    component: EventPeekPanel,
    parameters: { layout: 'fullscreen' },
    args: { event: EVENT_HOSTED },
    beforeEach: () =>
      mockApi({
        updateParticipation: () => delay(undefined),
        deleteEvent: () => delay(undefined),
        nudgeNoAnswers: () => delay({ nudged: 1, dms_sent: 1, discord_failed: false })
      })
  });
</script>

<Story name="Hosted by me" />
<Story name="Invited, awaiting my answer" args={{ event: EVENT_INVITED }} />
<Story name="Discovered (public, not invited)" args={{ event: EVENT_DISCOVERED }} />
<Story name="Nothing selected" args={{ event: null }} />
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

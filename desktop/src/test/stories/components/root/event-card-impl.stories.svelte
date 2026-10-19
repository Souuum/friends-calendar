<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventCardImpl from '$lib/components/EventCardImpl.svelte';
  import { EVENT_HOSTED, EVENT_INVITED } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  const { Story } = defineMeta({
    title: 'Components/EventCardImpl',
    component: EventCardImpl,
    args: { event: EVENT_HOSTED },
    beforeEach: () =>
      mockApi({ updateParticipation: () => delay(undefined), deleteEvent: () => delay(undefined) })
  });
</script>

<Story name="Hosted by me" />
<Story name="Invited" args={{ event: EVENT_INVITED }} />

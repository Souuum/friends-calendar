<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventRsvpCard from '$lib/components/molecules/EventRsvpCard.svelte';
  import { EVENT_HOSTED, EVENT_INVITED, EVENT_DECLINED, EVENT_LONG_TITLE } from '../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Molecules/EventRsvpCard',
    component: EventRsvpCard,
    args: { event: EVENT_HOSTED }
  });
</script>

<Story name="Going, with price and link" />
<Story name="Awaiting answer" args={{ event: EVENT_INVITED }} />
<Story name="Declined" args={{ event: EVENT_DECLINED }} />

<!-- The unbroken URL here once widened /friends/[id] past the viewport. -->
<Story name="Narrow, long content">
  <div class="w-[360px]">
    <EventRsvpCard
      event={{
        ...EVENT_LONG_TITLE,
        link: 'https://example.com/a/rather/long/link/that/should/not/overflow'
      }}
    />
  </div>
</Story>

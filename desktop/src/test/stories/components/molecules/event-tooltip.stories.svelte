<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventTooltip from '$lib/components/molecules/EventTooltip.svelte';
  import { EVENT_HOSTED, EVENT_INVITED } from '../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Molecules/EventTooltip',
    component: EventTooltip,
    args: { events: [EVENT_HOSTED], isVisible: true, position: { x: 24, y: 24 } }
  });
</script>

<Story name="One event" />
<Story name="Several events" args={{ events: [EVENT_HOSTED, EVENT_INVITED] }} />
<Story name="Hidden" args={{ isVisible: false }} />

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CompactEvent from '$lib/components/atoms/event/CompactEvent.svelte';
  import {
    EVENT_HOSTED,
    EVENT_INVITED,
    EVENT_MAYBE,
    EVENT_DECLINED,
    EVENT_DISCOVERED,
    EVENT_LONG_TITLE
  } from '../../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/CompactEvent',
    component: CompactEvent,
    args: { event: EVENT_HOSTED },
    parameters: { layout: 'padded' }
  });
</script>

<Story name="Going" />
<Story name="Awaiting answer" args={{ event: EVENT_INVITED }} />
<Story name="Maybe" args={{ event: EVENT_MAYBE }} />
<Story name="Declined" args={{ event: EVENT_DECLINED }} />
<Story name="Discovered (not invited)" args={{ event: EVENT_DISCOVERED }} />

<Story name="Long title in a narrow cell">
  <div class="w-32">
    <CompactEvent event={EVENT_LONG_TITLE} />
  </div>
</Story>

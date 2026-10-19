<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import EventList from '$lib/components/molecules/EventList.svelte';
  import { EVENTS } from '../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Molecules/EventList',
    component: EventList,
    argTypes: {
      variant: { control: 'inline-radio', options: ['compact', 'default', 'detailed', 'card'] }
    },
    args: { events: EVENTS, onEventClick: fn() }
  });
</script>

<Story name="Default (capped at 3)" />
<Story name="Compact" args={{ variant: 'compact' }} />
<Story name="Detailed" args={{ variant: 'detailed' }} />
<Story name="Show more" args={{ showMore: true }} />
<Story name="Empty" args={{ events: [] }} />

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import AgendaView from '$lib/components/organisms/AgendaView.svelte';
  import { EVENTS } from '../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Organisms/AgendaView',
    component: AgendaView,
    args: { events: EVENTS, onEventClick: fn() }
  });
</script>

<Story name="Upcoming events" />
<Story name="Empty" args={{ events: [] }} />

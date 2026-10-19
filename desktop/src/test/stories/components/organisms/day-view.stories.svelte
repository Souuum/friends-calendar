<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import DayView from '$lib/components/organisms/DayView.svelte';
  import { BUSY, EVENTS, eventsForDayOf } from '../../fixtures';

  const today = new Date();

  const { Story } = defineMeta({
    title: 'Components/Organisms/DayView',
    component: DayView,
    parameters: { layout: 'fullscreen' },
    args: { day: today, events: eventsForDayOf(EVENTS)(today), onEventClick: fn() }
  });
</script>

<Story name="With events" />
<Story name="With imported busy blocks" args={{ busy: BUSY }} />
<Story name="Empty day" args={{ events: [] }} />

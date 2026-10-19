<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import WeekView from '$lib/components/organisms/WeekView.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { BUSY, EVENTS, eventsForDayOf } from '../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Organisms/WeekView',
    component: WeekView,
    parameters: { layout: 'fullscreen' },
    args: {
      weekDays: dateUtils.getWeekDays(new Date()),
      eventsForDay: eventsForDayOf(EVENTS),
      onEventClick: fn()
    }
  });
</script>

<Story name="With events" />
<Story name="With imported busy blocks" args={{ busy: BUSY }} />
<Story name="Mobile (day strip)" globals={{ viewport: { value: 'mobile1', isRotated: false } }} />

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import MonthView from '$lib/components/organisms/MonthView.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { BUSY, EVENTS, eventsForDayOf } from '../../fixtures';

  const today = new Date();

  const { Story } = defineMeta({
    title: 'Components/Organisms/MonthView',
    component: MonthView,
    parameters: { layout: 'fullscreen' },
    args: {
      monthGrid: dateUtils.getMonthGrid(today),
      currentMonth: today,
      eventsForDay: eventsForDayOf(EVENTS),
      onEventClick: fn(),
      onDayClick: fn(),
      onDayLongPress: fn()
    }
  });
</script>

<Story name="With events" />
<Story name="With imported busy blocks" args={{ busy: BUSY }} />
<Story name="Day selected" args={{ selectedDay: today }} />
<Story name="Empty month" args={{ eventsForDay: eventsForDayOf([]) }} />

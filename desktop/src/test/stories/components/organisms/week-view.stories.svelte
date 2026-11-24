<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import WeekView from '$lib/components/organisms/WeekView.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { BUSY, EVENTS, eventsForDayOf } from '../../fixtures';

  /**
   * The calendar's week view: seven days on a shared 24-hour grid, with events
   * at their hours and imported busy time as anonymous bands.
   *
   * ```svelte
   * <WeekView {weekDays} {eventsForDay} {busy} onEventClick={selectEvent} />
   * ```
   *
   * - Two layouts are always mounted and switched by CSS at `md:`: seven
   *   columns from `md:` up, and below it a strip of the seven days above one
   *   selected day's hours.
   * - The mobile layout selects today when the week contains it, otherwise the
   *   first day, and re-anchors whenever `weekDays` changes.
   * - Both grids open scrolled to the first event rather than to midnight.
   * - `weekDays` comes from `dateUtils.getWeekDays(date)`.
   * - Dispatches no events: a clicked event calls `onEventClick`.
   */
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

<!-- The current week from `weekDays`, with events from `eventsForDay` and no `busy` blocks. -->
<Story name="With events" />

<!-- `busy` set: imported calendar time is drawn as bands behind the events. -->
<Story name="With imported busy blocks" args={{ busy: BUSY }} />

<!-- The same props in a phone viewport: the day strip with one day's hours under it. -->
<Story
  name="Mobile (day strip)"
  globals={{ viewport: { value: 'mobile1', isRotated: false } }}
  parameters={{ docs: { story: { inline: false, height: '600px' } } }}
/>

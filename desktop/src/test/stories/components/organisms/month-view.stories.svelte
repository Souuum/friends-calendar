<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import MonthView from '$lib/components/organisms/MonthView.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { BUSY, EVENTS, eventsForDayOf } from '../../fixtures';

  const today = new Date();

  /**
   * The calendar's month grid: one cell per day with up to three event chips,
   * a "+N more" line past that, and a "N busy" summary for imported calendar
   * time.
   *
   * ```svelte
   * <MonthView
   *   bind:this={monthViewRef}
   *   {monthGrid}
   *   {busy}
   *   currentMonth={currentDate}
   *   {eventsForDay}
   *   {selectedDay}
   *   onDayClick={toggleDay}
   *   onDayLongPress={startEventOn}
   *   onEventClick={selectEvent}
   *   on:showTooltip={handleShowTooltip}
   *   on:hideTooltip={handleHideTooltip}
   * />
   * ```
   *
   * - A click, Enter or Space on a cell calls `onDayClick`; a touch long-press
   *   or a double-click calls `onDayLongPress`; a chip calls `onEventClick`
   *   without also selecting the day.
   * - Dispatches `showTooltip` with `{ day, events, position }` after hovering
   *   a day that has events for 500ms, and `hideTooltip` shortly after the
   *   pointer leaves. It draws no tooltip itself - the parent does, and calls
   *   the exported `handleTooltipMouseEnter()` / `handleTooltipMouseLeave()`
   *   (through `bind:this`) so the tooltip stays up while hovered.
   * - `monthGrid` comes from `dateUtils.getMonthGrid(date)`. Days outside
   *   `currentMonth` and days in the past are dimmed.
   */
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

<!-- The current month with events from `eventsForDay`, no `selectedDay` and no `busy` blocks. -->
<Story name="With events" />

<!-- `busy` set: days with imported calendar time get a "N busy" summary. -->
<Story name="With imported busy blocks" args={{ busy: BUSY }} />

<!-- `selectedDay` set to today: that cell is highlighted. -->
<Story name="Day selected" args={{ selectedDay: today }} />

<!-- `eventsForDay` returning nothing for every day: an empty grid. -->
<Story name="Empty month" args={{ eventsForDay: eventsForDayOf([]) }} />

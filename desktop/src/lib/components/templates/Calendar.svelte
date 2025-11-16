<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import CalendarHeader from '../molecules/CalendarHeader.svelte';
  import MonthView from '../organisms/MonthView.svelte';
  import WeekView from '../organisms/WeekView.svelte';
  import DayView from '../organisms/DayView.svelte';
  import EventTooltip from '../molecules/EventTooltip.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { createEventDispatcher } from 'svelte';

  export let events: EventWithParticipants[] = [];

  const dispatch = createEventDispatcher();

  type ViewType = 'month' | 'week' | 'day';
  let view: ViewType = 'month';
  let currentDate = new Date();

  // Tooltip state
  let tooltipVisible = false;
  let tooltipEvents: EventWithParticipants[] = [];
  let tooltipPosition = { x: 0, y: 0 };
  let tooltipElement: HTMLElement;

  function handleShowTooltip(event: CustomEvent) {
    tooltipEvents = event.detail.events;
    tooltipPosition = event.detail.position;
    tooltipVisible = true;
  }

  function handleHideTooltip() {
    tooltipVisible = false;
  }

  function handleRefresh() {
    dispatch('refresh');
  }

  // Navigation
  function prev(): void {
    const newDate = new Date(currentDate);
    if (view === 'month') {
      newDate.setMonth(newDate.getMonth() - 1);
    } else if (view === 'week') {
      newDate.setDate(newDate.getDate() - 7);
    } else {
      newDate.setDate(newDate.getDate() - 1);
    }
    currentDate = newDate;
  }

  function next(): void {
    const newDate = new Date(currentDate);
    if (view === 'month') {
      newDate.setMonth(newDate.getMonth() + 1);
    } else if (view === 'week') {
      newDate.setDate(newDate.getDate() + 7);
    } else {
      newDate.setDate(newDate.getDate() + 1);
    }
    currentDate = newDate;
  }

  function goToToday(): void {
    currentDate = new Date();
  }

  function handleViewChange(newView: ViewType): void {
    view = newView;
  }

  function eventsForDay(day: Date): EventWithParticipants[] {
    return events.filter((event) => {
      const eventDate = new Date(event.start_time);
      return dateUtils.isSameDay(eventDate, day);
    });
  }

  function getMonthGrid(): Date[] {
    return dateUtils.getMonthGrid(currentDate);
  }

  function getWeekDays(): Date[] {
    return dateUtils.getWeekDays(currentDate);
  }

  function formatHeaderDate(): string {
    if (view === 'month') {
      return currentDate.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
    } else if (view === 'week') {
      const weekDays = getWeekDays();
      const start = weekDays[0];
      const end = weekDays[6];
      return `${start.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })} - ${end.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })}`;
    } else {
      return currentDate.toLocaleDateString('en-US', {
        weekday: 'long',
        month: 'long',
        day: 'numeric',
        year: 'numeric'
      });
    }
  }
</script>

<CalendarHeader
  title={formatHeaderDate()}
  {view}
  onPrev={prev}
  onNext={next}
  onToday={goToToday}
  onViewChange={handleViewChange}
/>

{#if view === 'month'}
  <MonthView
    monthGrid={getMonthGrid()}
    currentMonth={currentDate}
    {eventsForDay}
    on:showTooltip={handleShowTooltip}
    on:hideTooltip={handleHideTooltip}
  />
{:else if view === 'week'}
  <WeekView
    weekDays={getWeekDays()}
    {eventsForDay}
    on:showTooltip={handleShowTooltip}
    on:hideTooltip={handleHideTooltip}
  />
{:else}
  <DayView events={eventsForDay(currentDate)} />
{/if}

<!-- Tooltip with EventCards -->
<div
  bind:this={tooltipElement}
  on:mouseenter={() => (tooltipVisible = true)}
  on:mouseleave={handleHideTooltip}
  role="tooltip"
  tabindex="-1"
>
  <EventTooltip
    events={tooltipEvents}
    isVisible={tooltipVisible}
    position={tooltipPosition}
    on:refresh={handleRefresh}
  />
</div>

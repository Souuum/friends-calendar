<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import CalendarHeader from '$lib/components/molecules/CalendarHeader.svelte';
  import MonthView from '$lib/components/organisms/MonthView.svelte';
  import WeekView from '$lib/components/organisms/WeekView.svelte';
  import DayView from '$lib/components/organisms/DayView.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { createEventDispatcher } from 'svelte';

  export let events: EventWithParticipants[] = [];

  const dispatch = createEventDispatcher();

  type ViewType = 'month' | 'week' | 'day';
  let view: ViewType = 'month';
  let currentDate = new Date();

  // Modal state
  let selectedEvent: EventWithParticipants | null = null;
  let isModalOpen = false;

  function openEventDetails(event: EventWithParticipants) {
    selectedEvent = event;
    isModalOpen = true;
  }

  function handleRefresh() {
    dispatch('refresh');
  }

  // $lib/components. (keep all the existing navigation and utility functions)

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
    onViewChange={handleViewChange} />

  {#if view === 'month'}
    <MonthView
      monthGrid={getMonthGrid()}
      currentMonth={currentDate}
      {eventsForDay}
      onEventClick={openEventDetails} />
  {:else if view === 'week'}
    <WeekView weekDays={getWeekDays()} {eventsForDay} onEventClick={openEventDetails} />
  {:else}
    <DayView events={eventsForDay(currentDate)} />
  {/if}

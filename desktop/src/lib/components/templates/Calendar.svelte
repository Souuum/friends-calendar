<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import CalendarHeader from '$lib/components/molecules/CalendarHeader.svelte';
  import MonthView from '$lib/components/organisms/MonthView.svelte';
  import WeekView from '$lib/components/organisms/WeekView.svelte';
  import DayView from '$lib/components/organisms/DayView.svelte';
  import EventTooltip from '$lib/components/molecules/EventTooltip.svelte';
  import EventDetailsModal from '$lib/components/organisms/EventDetailsModal.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import { createEventDispatcher } from 'svelte';

  export let events: EventWithParticipants[] = [];

  const dispatch = createEventDispatcher();

  type ViewType = 'month' | 'week' | 'day';
  let view: ViewType = 'month';
  let currentDate = new Date();

  // Tooltip state (for month view)
  let tooltipVisible = false;
  let tooltipEvents: EventWithParticipants[] = [];
  let tooltipPosition = { x: 0, y: 0 };
  let monthViewRef: MonthView;

  // Modal state (for week/day view)
  let selectedEvent: EventWithParticipants | null = null;
  let isModalOpen = false;

  function handleShowTooltip(event: CustomEvent) {
    tooltipEvents = event.detail.events;
    tooltipPosition = event.detail.position;
    tooltipVisible = true;
  }

  function handleHideTooltip() {
    tooltipVisible = false;
  }

  function handleTooltipMouseEnter() {
    if (monthViewRef) {
      monthViewRef.handleTooltipMouseEnter();
    }
  }

  function handleTooltipMouseLeave() {
    if (monthViewRef) {
      monthViewRef.handleTooltipMouseLeave();
    }
  }

  function openEventDetails(event: EventWithParticipants) {
    selectedEvent = event;
    isModalOpen = true;
  }

  function closeModal() {
    isModalOpen = false;
    selectedEvent = null;
  }

  function handleRefresh() {
    dispatch('refresh');
  }

  function prev(): void {
    if (view === 'month') {
      currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() - 1, 1);
    } else if (view === 'week') {
      const newDate = new Date(currentDate);
      newDate.setDate(newDate.getDate() - 7);
      currentDate = newDate;
    } else {
      const newDate = new Date(currentDate);
      newDate.setDate(newDate.getDate() - 1);
      currentDate = newDate;
    }
    console.log('Previous date:', currentDate);
  }

  function next(): void {
    if (view === 'month') {
      currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() + 1, 1);
    } else if (view === 'week') {
      const newDate = new Date(currentDate);
      newDate.setDate(newDate.getDate() + 7);
      currentDate = newDate;
    } else {
      const newDate = new Date(currentDate);
      newDate.setDate(newDate.getDate() + 1);
      currentDate = newDate;
    }
    console.log('Next date:', currentDate);
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


  $: monthGrid = dateUtils.getMonthGrid(currentDate);
  $: weekDays = dateUtils.getWeekDays(currentDate);
  $: headerDate = (() => {
    if (view === 'day') {
      return currentDate.toLocaleDateString('en-US', {
        weekday: 'long',
        month: 'long',
        day: 'numeric',
        year: 'numeric'
      });
    } else {
      return currentDate.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
    }
  })();
</script>

<CalendarHeader
  title={headerDate}
  {view}
  onPrev={prev}
  onNext={next}
  onToday={goToToday}
  onViewChange={handleViewChange}
/>

{#if view === 'month'}
  <MonthView
    bind:this={monthViewRef}
    {monthGrid}
    currentMonth={currentDate}
    {eventsForDay}
    on:showTooltip={handleShowTooltip}
    on:hideTooltip={handleHideTooltip}
  />

  <EventTooltip
    events={tooltipEvents}
    isVisible={tooltipVisible}
    position={tooltipPosition}
    on:mouseenter={handleTooltipMouseEnter}
    on:mouseleave={handleTooltipMouseLeave}
    on:refresh={handleRefresh}
  />
{:else if view === 'week'}
  <WeekView {weekDays} {eventsForDay} onEventClick={openEventDetails} />
{:else}
  <DayView {currentDate} events={eventsForDay(currentDate)} onEventClick={openEventDetails} />
{/if}

<EventDetailsModal
  event={selectedEvent}
  isOpen={isModalOpen}
  on:close={closeModal}
  on:refresh={handleRefresh}
/>

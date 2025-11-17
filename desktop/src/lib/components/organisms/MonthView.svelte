<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import WeekdayHeader from '$lib/components/atoms/WeekdayHeader.svelte';
  import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';
  import EventList from '$lib/components/molecules/EventList.svelte';
  import { createEventDispatcher } from 'svelte';
  import { calculateTooltipPosition } from '$lib/utils/tooltipUtils';

  export let monthGrid: Date[];
  export let currentMonth: Date;
  export let eventsForDay: (day: Date) => EventWithParticipants[];

  const dispatch = createEventDispatcher();

  let hoverTimer: ReturnType<typeof setTimeout> | null = null;
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let hoveredDay: Date | null = null;
  let isTooltipHovered = false;

  function isToday(date: Date): boolean {
    const today = new Date();
    return (
      date.getFullYear() === today.getFullYear() &&
      date.getMonth() === today.getMonth() &&
      date.getDate() === today.getDate()
    );
  }

  function isCurrentMonth(date: Date): boolean {
    return date.getMonth() === currentMonth.getMonth();
  }

  function handleMouseEnter(day: Date, e: MouseEvent) {
    const dayEvents = eventsForDay(day);
    if (dayEvents.length === 0) return;

    const target = e.currentTarget as HTMLElement | null;
    if (!target) return;

    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }

    if (hoverTimer) {
      clearTimeout(hoverTimer);
    }

    hoverTimer = setTimeout(() => {
      hoveredDay = day;

      const rect = target.getBoundingClientRect();
      const tooltipPosition = calculateTooltipPosition(rect);

      dispatch('showTooltip', { day, events: dayEvents, position: tooltipPosition });
    }, 1000);
  }

  function handleMouseLeave() {
    if (hoverTimer) {
      clearTimeout(hoverTimer);
      hoverTimer = null;
    }

    hideTimer = setTimeout(() => {
      if (!isTooltipHovered) {
        hoveredDay = null;
        dispatch('hideTooltip');
      }
    }, 150);
  }

  function handleTooltipMouseEnter() {
    isTooltipHovered = true;
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
  }

  function handleTooltipMouseLeave() {
    isTooltipHovered = false;
    hideTimer = setTimeout(() => {
      hoveredDay = null;
      dispatch('hideTooltip');
    }, 150);
  }

  export { handleTooltipMouseEnter, handleTooltipMouseLeave };
</script>

<div class="p-6">
  <WeekdayHeader />

  <div class="grid grid-cols-7 gap-px bg-gray-200 rounded-lg overflow-hidden">
    {#each monthGrid as day, i}
      {@const dayEvents = eventsForDay(day)}
      <div
        role="button"
        tabindex="0"
        class="bg-white min-h-[120px] p-3 hover:bg-gray-50 transition-colors cursor-pointer relative"
        class:opacity-40={!isCurrentMonth(day)}
        class:rounded-tl-lg={i === 0}
        class:rounded-tr-lg={i === 6}
        class:rounded-bl-lg={i === 35}
        class:rounded-br-lg={i === 41}
        on:mouseenter={(e) => handleMouseEnter(day, e)}
        on:mouseleave={handleMouseLeave}
      >
        <div class="flex justify-between items-start mb-2">
          <CalendarDay date={day} isToday={isToday(day)} isCurrentMonth={isCurrentMonth(day)} />
        </div>

        <EventList events={dayEvents} variant="compact" maxVisible={3} showMore={true} />
      </div>
    {/each}
  </div>
</div>

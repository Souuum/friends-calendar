<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import WeekdayHeader from '$lib/components/atoms/WeekdayHeader.svelte';
  import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';
  import EventList from '$lib/components/molecules/EventList.svelte';
  import { calculateTooltipPosition } from '$lib/utils/tooltipUtils';
  import { createEventDispatcher } from 'svelte';

  export let monthGrid: Date[];
  export let currentMonth: Date;
  export let eventsForDay: (day: Date) => EventWithParticipants[];

  const dispatch = createEventDispatcher();

  let hoverTimer: ReturnType<typeof setTimeout> | null = null;
  let hoveredDay: Date | null = null;
  let tooltipPosition = { x: 0, y: 0 };

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

    // Clear any existing timer
    if (hoverTimer) {
      clearTimeout(hoverTimer);
    }

    // Set a 1-second delay before showing tooltip
    hoverTimer = setTimeout(() => {
      hoveredDay = day;

      // Calculate position with smart positioning
      const rect = target.getBoundingClientRect();
      tooltipPosition = calculateTooltipPosition(rect);

      dispatch('showTooltip', { day, events: dayEvents, position: tooltipPosition });
    }, 1000); // 1 second delay
  }

  function handleMouseLeave() {
    // Clear the timer if user leaves before 1 second
    if (hoverTimer) {
      clearTimeout(hoverTimer);
      hoverTimer = null;
    }

    // Small delay before hiding to allow moving to tooltip
    setTimeout(() => {
      hoveredDay = null;
      dispatch('hideTooltip');
    }, 100);
  }
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

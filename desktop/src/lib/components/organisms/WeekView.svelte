<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';
  import EventList from '$lib/components/molecules/EventList.svelte';
  import { calculateTooltipPosition } from '$lib/utils/tooltipUtils';
  import { createEventDispatcher } from 'svelte';

  export let weekDays: Date[];
  export let eventsForDay: (day: Date) => EventWithParticipants[];

  const dispatch = createEventDispatcher();

  let hoverTimer: ReturnType<typeof setTimeout> | null = null;

  function isToday(date: Date): boolean {
    const today = new Date();
    return (
      date.getFullYear() === today.getFullYear() &&
      date.getMonth() === today.getMonth() &&
      date.getDate() === today.getDate()
    );
  }

  function handleMouseEnter(day: Date, e: MouseEvent) {
    const dayEvents = eventsForDay(day);
    if (dayEvents.length === 0) return;

    const target = e.currentTarget as HTMLElement | null;
    if (!target) return;

    if (hoverTimer) {
      clearTimeout(hoverTimer);
    }

    hoverTimer = setTimeout(() => {
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

    setTimeout(() => {
      dispatch('hideTooltip');
    }, 100);
  }
</script>

<div class="p-6">
  <!-- Weekday headers with dates -->
  <div class="grid grid-cols-7 mb-2">
    {#each weekDays as day}
      <div class="text-center py-3">
        <div
          class="inline-flex flex-col items-center"
          class:text-primary={isToday(day)}
          class:font-semibold={isToday(day)}
        >
          <span class="text-sm font-medium text-gray-600">
            {day.toLocaleDateString('en-US', { weekday: 'short' })}
          </span>
          <CalendarDay date={day} isToday={isToday(day)} size="large" />
        </div>
      </div>
    {/each}
  </div>

  <!-- Week grid -->
  <div class="grid grid-cols-7 gap-px bg-gray-200 rounded-lg overflow-hidden">
    {#each weekDays as day}
      {@const dayEvents = eventsForDay(day)}
      <div
        role="button"
        tabindex="0"
        class="bg-white min-h-[300px] p-3 hover:bg-gray-50 transition-colors"
        on:mouseenter={(e) => handleMouseEnter(day, e)}
        on:mouseleave={handleMouseLeave}
      >
        <div class="space-y-2 overflow-y-auto max-h-[280px]">
          <EventList events={dayEvents} variant="default" />
        </div>
      </div>
    {/each}
  </div>
</div>

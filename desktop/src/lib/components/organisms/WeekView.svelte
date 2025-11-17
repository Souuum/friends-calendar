<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';
  import TimeLabel from '$lib/components/atoms/TimeLabel.svelte';
  import TimeSlot from '$lib/components/atoms/TimeSlot.svelte';
  import TimedEvent from '$lib/components/molecules/TimedEvent.svelte';
  import { createEventDispatcher } from 'svelte';

  export let weekDays: Date[];
  export let eventsForDay: (day: Date) => EventWithParticipants[];
  export let onEventClick: ((event: EventWithParticipants) => void) | undefined = undefined;

  const dispatch = createEventDispatcher();
  const hours = Array.from({ length: 24 }, (_, i) => i);

  function isToday(date: Date): boolean {
    const today = new Date();
    return (
      date.getFullYear() === today.getFullYear() &&
      date.getMonth() === today.getMonth() &&
      date.getDate() === today.getDate()
    );
  }

  function getEventsForHour(day: Date, hour: number): EventWithParticipants[] {
    const dayEvents = eventsForDay(day);
    return dayEvents.filter((event) => {
      const eventStart = new Date(event.start_time);
      const eventHour = eventStart.getHours();
      return eventHour === hour;
    });
  }
</script>

<div class="p-6">
  <div class="grid grid-cols-[80px_repeat(7,1fr)] gap-0 mb-2 sticky top-0 bg-white z-10">
    <div></div>
    <!-- Empty cell for time column -->
    {#each weekDays as day}
      <div class="text-center py-3 border-b border-gray-200">
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

  <div class="overflow-y-auto max-h-[calc(100vh-300px)]">
    <div class="grid grid-cols-[80px_repeat(7,1fr)] gap-0">
      {#each hours as hour}
        <!-- Time label -->
        <div class="flex items-start pt-2">
          <TimeLabel {hour} />
        </div>
        {#each weekDays as day}
          {@const hourEvents = getEventsForHour(day, hour)}
          <TimeSlot {hour} hasEvents={hourEvents.length > 0}>
            {#each hourEvents as event}
              <TimedEvent {event} onClick={() => onEventClick?.(event)} />
            {/each}
          </TimeSlot>
        {/each}
      {/each}
    </div>
  </div>
</div>

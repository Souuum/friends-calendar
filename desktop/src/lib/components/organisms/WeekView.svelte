<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';
  import EventList from '$lib/components/molecules/EventList.svelte';

  export let weekDays: Date[];
  export let eventsForDay: (day: Date) => EventWithParticipants[];
  export let onEventClick: ((event: EventWithParticipants) => void) | undefined = undefined;

  function isToday(date: Date): boolean {
    const today = new Date();
    return (
      date.getFullYear() === today.getFullYear() &&
      date.getMonth() === today.getMonth() &&
      date.getDate() === today.getDate()
    );
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
          class:font-semibold={isToday(day)}>
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
      <div class="bg-white min-h-[300px] p-3 hover:bg-gray-50 transition-colors">
        <div class="space-y-2 overflow-y-auto max-h-[280px]">
          <EventList events={dayEvents} variant="default" {onEventClick} />
        </div>
      </div>
    {/each}
  </div>
</div>
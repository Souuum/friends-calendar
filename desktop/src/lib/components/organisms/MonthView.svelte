<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import WeekdayHeader from '$lib/components/atoms/WeekdayHeader.svelte';
  import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';
  import EventList from '$lib/components/molecules/EventList.svelte';

  export let monthGrid: Date[];
  export let currentMonth: Date;
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

  function isCurrentMonth(date: Date): boolean {
    return date.getMonth() === currentMonth.getMonth();
  }
</script>

<div class="p-6">
  <WeekdayHeader />

  <div class="grid grid-cols-7 gap-px bg-gray-200 rounded-lg overflow-hidden">
    {#each monthGrid as day, i}
      {@const dayEvents = eventsForDay(day)}
      <div
        class="bg-white min-h-[120px] p-3 hover:bg-gray-50 transition-colors cursor-pointer"
        class:opacity-40={!isCurrentMonth(day)}
        class:rounded-tl-lg={i === 0}
        class:rounded-tr-lg={i === 6}
        class:rounded-bl-lg={i === 35}
        class:rounded-br-lg={i === 41}>
        <div class="flex justify-between items-start mb-2">
          <CalendarDay date={day} isToday={isToday(day)} isCurrentMonth={isCurrentMonth(day)} />
        </div>

        <EventList
          events={dayEvents}
          variant="compact"
          maxVisible={3}
          showMore={true}
          {onEventClick} />
      </div>
    {/each}
  </div>
</div>
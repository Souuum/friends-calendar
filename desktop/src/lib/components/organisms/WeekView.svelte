<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';
  import TimeLabel from '$lib/components/atoms/TimeLabel.svelte';
  import TimeSlot from '$lib/components/atoms/TimeSlot.svelte';
  import TimedEvent from '$lib/components/molecules/TimedEvent.svelte';
  import { anchorToFirstEvent } from '$lib/utils/timeGrid';

  export let weekDays: Date[];
  export let eventsForDay: (day: Date) => EventWithParticipants[];
  export let onEventClick: ((event: EventWithParticipants) => void) | undefined = undefined;

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
    return eventsForDay(day).filter((event) => new Date(event.start_time).getHours() === hour);
  }

  /**
   * Which day the narrow layout is showing.
   *
   * Seven columns leave about 39px each at 402px, which cannot hold an
   * event chip - so below `md:` this becomes a day strip plus one day's
   * hours. The week is still the unit of navigation; only the display
   * narrows.
   */
  let selectedIndex = 0;

  // Depends on weekDays alone, so paging to another week re-anchors on
  // today (or the week's start), while tapping a day inside the current
  // week is left alone.
  $: (weekDays, (selectedIndex = defaultIndexFor(weekDays)));

  function defaultIndexFor(days: Date[]): number {
    const today = days.findIndex(isToday);
    return today >= 0 ? today : 0;
  }

  $: selectedDay = weekDays[selectedIndex] ?? weekDays[0];
  $: selectedDayEvents = selectedDay ? eventsForDay(selectedDay) : [];
</script>

<!-- Narrow: a day strip over a single day's hours. -->
<div class="md:hidden p-3" data-testid="week-view-mobile">
  <div class="grid grid-cols-7 gap-1 mb-3">
    {#each weekDays as day, i (day.toISOString())}
      {@const active = i === selectedIndex}
      <button
        on:click={() => (selectedIndex = i)}
        aria-pressed={active}
        aria-label={day.toLocaleDateString('en-US', {
          weekday: 'long',
          month: 'long',
          day: 'numeric'
        })}
        class="flex flex-col items-center gap-0.5 py-2 rounded-lg border transition-colors {active
          ? 'border-primary bg-tint'
          : 'border-transparent hover:bg-gray-100'}"
      >
        <span class="text-[11px] font-medium text-gray-500">
          {day.toLocaleDateString('en-US', { weekday: 'narrow' })}
        </span>
        <CalendarDay date={day} isToday={isToday(day)} size="small" />
      </button>
    {/each}
  </div>

  {#if selectedDayEvents.length === 0}
    <p class="text-center text-sm text-gray-500 py-8 m-0">Nothing on this day.</p>
  {:else}
    <div class="overflow-y-auto max-h-[60vh]" use:anchorToFirstEvent={selectedDayEvents}>
      <div class="grid grid-cols-[56px_1fr] gap-0">
        {#each hours as hour}
          <div class="flex items-start pt-2"><TimeLabel {hour} compact /></div>
          {@const hourEvents = selectedDay ? getEventsForHour(selectedDay, hour) : []}
          <TimeSlot {hour} hasEvents={hourEvents.length > 0}>
            {#each hourEvents as event (event.id)}
              <TimedEvent {event} onClick={() => onEventClick?.(event)} />
            {/each}
          </TimeSlot>
        {/each}
      </div>
    </div>
  {/if}
</div>

<!-- Wide: the real seven-column week, unchanged. -->
<div class="hidden md:block p-6" data-testid="week-view-desktop">
  <div class="grid grid-cols-[80px_repeat(7,1fr)] gap-0 mb-2 sticky top-0 bg-surface z-10">
    <div></div>
    {#each weekDays as day (day.toISOString())}
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
        <div class="flex items-start pt-2">
          <TimeLabel {hour} />
        </div>
        {#each weekDays as day (day.toISOString())}
          {@const hourEvents = getEventsForHour(day, hour)}
          <TimeSlot {hour} hasEvents={hourEvents.length > 0}>
            {#each hourEvents as event (event.id)}
              <TimedEvent {event} onClick={() => onEventClick?.(event)} />
            {/each}
          </TimeSlot>
        {/each}
      {/each}
    </div>
  </div>
</div>

<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import TimeLabel from '$lib/components/atoms/TimeLabel.svelte';
  import TimeSlot from '$lib/components/atoms/TimeSlot.svelte';
  import TimedEvent from '$lib/components/molecules/TimedEvent.svelte';

  export let currentDate: Date = new Date();
  export let events: EventWithParticipants[];
  export let onEventClick: ((event: EventWithParticipants) => void) | undefined = undefined;

  const hours = Array.from({ length: 24 }, (_, i) => i);

  function getEventsForHour(hour: number): EventWithParticipants[] {
    return events.filter((event) => {
      const eventStart = new Date(event.start_time);
      const eventHour = eventStart.getHours();
      return eventHour === hour;
    });
  }
</script>

<div class="p-6">
  <div class="text-center mb-6">
    <h3 class="text-xl font-semibold">
      {currentDate.toLocaleDateString('en-US', {
        weekday: 'long',
        month: 'long',
        day: 'numeric',
        year: 'numeric'
      })}
    </h3>
  </div>

  <div class="overflow-y-auto max-h-[calc(100vh-300px)]">
    <div class="grid grid-cols-[80px_1fr] gap-0">
      {#each hours as hour}
        <!-- Time label -->
        <div class="flex items-start pt-2">
          <TimeLabel {hour} />
        </div>
        {@const hourEvents = getEventsForHour(hour)}
        <TimeSlot {hour} hasEvents={hourEvents.length > 0}>
          {#each hourEvents as event}
            <TimedEvent {event} onClick={() => onEventClick?.(event)} />
          {/each}
        </TimeSlot>
      {/each}
    </div>
  </div>

  {#if events.length === 0}
    <div class="text-center py-12 text-gray-500">
      <svg
        class="w-16 h-16 mx-auto mb-4 text-gray-300"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
        />
      </svg>
      <p class="text-lg font-medium">No events today</p>
    </div>
  {/if}
</div>

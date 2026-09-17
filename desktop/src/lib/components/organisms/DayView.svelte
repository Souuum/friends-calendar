<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import Icon from '$lib/components/atoms/Icon.svelte';
  import TimeLabel from '$lib/components/atoms/TimeLabel.svelte';
  import TimeSlot from '$lib/components/atoms/TimeSlot.svelte';
  import TimedEvent from '$lib/components/molecules/TimedEvent.svelte';
  import { anchorToFirstEvent } from '$lib/utils/timeGrid';

  export let events: EventWithParticipants[];
  export let onEventClick: ((event: EventWithParticipants) => void) | undefined = undefined;

  const hours = Array.from({ length: 24 }, (_, i) => i);

  function getEventsForHour(hour: number): EventWithParticipants[] {
    return events.filter((event) => new Date(event.start_time).getHours() === hour);
  }
</script>

<!-- p-3 on mobile: p-6 alone eats 48px of a 402px screen, which the time
     gutter and the event chips both need more than the margin does. -->
<div class="p-3 md:p-6" data-testid="day-view">
  <!-- No date heading here: Calendar.svelte's `headerDate` already renders
       the identical "Thursday, September 17, 2026" above this component
       whenever view === 'day', so this was the same line twice. It costs
       real vertical space on a phone. -->

  {#if events.length === 0}
    <!-- Instead of the grid, not after it. This used to render *below* a
         full 24-hour grid, so an empty day meant scrolling past every hour
         of nothing to be told there was nothing. -->
    <div class="text-center py-12 text-gray-500">
      <Icon name="calendar" size={48} class="mx-auto mb-3 text-gray-300" />
      <p class="text-base font-medium m-0">No events today</p>
    </div>
  {:else}
    <!-- The action scrolls to the first event on mount and re-anchors when
         the day changes; all 24 hours stay reachable by scrolling. -->
    <div
      class="overflow-y-auto max-h-[60vh] md:max-h-[calc(100vh-300px)]"
      use:anchorToFirstEvent={events}
    >
      <div class="grid grid-cols-[56px_1fr] md:grid-cols-[80px_1fr] gap-0">
        {#each hours as hour}
          <div class="flex items-start pt-2">
            <span class="md:hidden"><TimeLabel {hour} compact /></span>
            <span class="hidden md:block"><TimeLabel {hour} /></span>
          </div>
          {@const hourEvents = getEventsForHour(hour)}
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

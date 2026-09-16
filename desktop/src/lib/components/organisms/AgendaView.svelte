<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import { groupEventsByDay } from '$lib/utils/dateUtils';

  // The chronological counterpart to MonthView/WeekView/DayView: upcoming
  // events grouped by day. This is what makes the calendar usable at 402px,
  // where a 7-column grid leaves ~55px per day.
  export let events: EventWithParticipants[] = [];
  export let onEventClick: ((event: EventWithParticipants) => void) | undefined = undefined;

  // Grid views are anchored to whatever month/week you're looking at; a list
  // has no such anchor, so it shows what's ahead rather than what's behind.
  $: groups = groupEventsByDay(events);

  function time(iso: string): string {
    return new Date(iso).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
  }
</script>

<div class="p-4 md:p-6">
  {#if groups.length === 0}
    <p class="text-sm text-gray-500">Nothing coming up.</p>
  {:else}
    <div class="flex flex-col gap-5">
      {#each groups as group (group.key)}
        <div>
          <h3 class="font-mono text-[10px] tracking-widest uppercase text-muted mb-2">
            {group.label}
          </h3>
          <div class="flex flex-col gap-1.5">
            {#each group.events as event (event.id)}
              <button
                on:click={() => onEventClick?.(event)}
                class="w-full text-left flex items-center gap-3 bg-white border rounded-[11px] px-3.5 py-3 hover:bg-gray-50 {event.is_participant
                  ? 'border-line'
                  : 'border-dashed border-line'}"
              >
                <span class="font-mono text-[11px] text-muted shrink-0 w-[52px]">
                  {time(event.start_time)}
                </span>
                <span class="flex-1 min-w-0 truncate text-[13px] font-medium text-gray-900">
                  {event.title}
                </span>
                {#if !event.is_participant}
                  <!-- Same distinction the month grid makes: an event you can
                       see but weren't invited to isn't one you owe an answer on. -->
                  <span class="text-[10px] text-muted shrink-0">Not invited</span>
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

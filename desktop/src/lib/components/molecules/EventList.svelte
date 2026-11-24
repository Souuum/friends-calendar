<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import CalendarEvent from '$lib/components/atoms/CalendarEvent.svelte';

  /** The events to list, in the order given. Only the first `maxVisible` are rendered. */
  export let events: EventWithParticipants[];
  /** How each event is drawn, passed to `CalendarEvent`: `compact`, `default`, `detailed` or `card`. */
  export let variant: 'compact' | 'default' | 'detailed' | 'card' = 'default';
  /** How many events to render before cutting the list off. */
  export let maxVisible = 3;
  /** Show a "+N more" line when there are more than `maxVisible` events. Off by default: the rest are dropped silently. */
  export let showMore = false;
  /** Called with the event when one is clicked. The click does not propagate to the parent. */
  export let onEventClick: ((event: EventWithParticipants) => void) | undefined = undefined;
</script>

<div class="flex w-full flex-col gap-0.5">
  {#each events.slice(0, maxVisible) as event}
    <!-- stopPropagation: the chip sits *inside* the day cell, and the cell
         selects the day on click. Without this, opening an event would also
         change which day is filtering the list - two actions from one tap.
         There's a test for exactly that. -->
    <div
      on:click|stopPropagation={() => onEventClick?.(event)}
      on:keydown|stopPropagation
      role="button"
      tabindex="0"
    >
      <CalendarEvent {event} {variant} />
    </div>
  {/each}

  {#if showMore && events.length > maxVisible}
    <span class="pl-[9px] text-[11px] text-muted">+{events.length - maxVisible} more</span>
  {/if}
</div>

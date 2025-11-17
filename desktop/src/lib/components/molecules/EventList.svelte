<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import CalendarEvent from '$lib/components/atoms/CalendarEvent.svelte';

  export let events: EventWithParticipants[];
  export let variant: 'compact' | 'default' | 'detailed' | 'card' = 'default';
  export let maxVisible = 3;
  export let showMore = false;
  export let onEventClick: ((event: EventWithParticipants) => void) | undefined = undefined;
</script>

<div class="space-y-1">
  {#each events.slice(0, maxVisible) as event}
    <div on:click={() => onEventClick?.(event)} on:keydown role="button" tabindex="0">
      <CalendarEvent {event} {variant} />
    </div>
  {/each}

  {#if showMore && events.length > maxVisible}
    <div class="text-xs text-gray-500 px-2">+{events.length - maxVisible} more</div>
  {/if}
</div>

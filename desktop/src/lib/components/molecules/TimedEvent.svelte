<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import { HOUR_HEIGHT } from '$lib/utils/timeGrid';

  export let event: EventWithParticipants;
  export let onClick: (() => void) | undefined = undefined;

  // HOUR_HEIGHT rather than a literal 80: it has to match TimeSlot's
  // `h-20`, and it was written out twice here with nothing tying the two
  // to the slot height they position against.
  function getEventHeight(event: EventWithParticipants): number {
    if (!event.end_time) return HOUR_HEIGHT;
    const start = new Date(event.start_time);
    const end = new Date(event.end_time);
    const duration = (end.getTime() - start.getTime()) / (1000 * 60);
    return (duration / 60) * HOUR_HEIGHT;
  }

  function getEventTop(event: EventWithParticipants): number {
    const minutes = new Date(event.start_time).getMinutes();
    return (minutes / 60) * HOUR_HEIGHT;
  }

  function formatTime(dateString: string): string {
    const date = new Date(dateString);
    return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
  }
</script>

<button
  on:click={onClick}
  class="absolute left-1 right-1 rounded-lg px-2 py-1 text-xs overflow-hidden text-left transition-all hover:shadow-md"
  style="background-color: {false || '#5030E5'}; 
         color: white;
         height: {getEventHeight(event)}px;
         top: {getEventTop(event)}px;
         min-height: 20px;"
>
  <div class="font-semibold truncate">{event.title || 'Untitled Event'}</div>
  <div class="text-[10px] opacity-90">
    {formatTime(event.start_time)}
  </div>
</button>

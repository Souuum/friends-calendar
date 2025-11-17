<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';

  export let event: EventWithParticipants;
  export let onClick: (() => void) | undefined = undefined;

  function getEventHeight(event: EventWithParticipants): number {
    if (!event.end_time) return 80; // Default 1 hour
    const start = new Date(event.start_time);
    const end = new Date(event.end_time);
    const duration = (end.getTime() - start.getTime()) / (1000 * 60); // minutes
    return (duration / 60) * 80; // 80px per hour
  }

  function getEventTop(event: EventWithParticipants): number {
    const start = new Date(event.start_time);
    const minutes = start.getMinutes();
    return (minutes / 60) * 80; // 80px per hour
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

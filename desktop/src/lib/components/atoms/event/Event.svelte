<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';

  export let event: EventWithParticipants;
</script>

<div class="text-xs px-2 py-2 bg-primary-hover bg-opacity-10 text-primary rounded">
  <div class="font-semibold">{event.title || 'Untitled Event'}</div>
  {#if event.start_time}
    <div class="text-gray-600 mt-1">
      {new Date(event.start_time).toLocaleTimeString('en-US', {
        hour: '2-digit',
        minute: '2-digit'
      })}
    </div>
  {/if}
</div>

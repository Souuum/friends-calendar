<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';

  export let event: EventWithParticipants;
  export let variant: 'compact' | 'default' | 'detailed' = 'default';
</script>

{#if variant === 'compact'}
  <div class="text-xs px-2 py-1 bg-primary bg-opacity-10 text-primary rounded truncate">
    {event.title || 'Untitled Event'}
  </div>
{:else if variant === 'default'}
  <div class="text-xs px-2 py-2 bg-primary bg-opacity-10 text-primary rounded">
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
{:else if variant === 'detailed'}
  <div
    class="p-4 bg-primary bg-opacity-10 border-l-4 border-primary rounded-lg hover:bg-opacity-20 transition-colors"
  >
    <h3 class="font-semibold text-lg mb-1">{event.title || 'Untitled Event'}</h3>
    {#if event.start_time}
      <p class="text-sm text-gray-600">
        {new Date(event.start_time).toLocaleTimeString('en-US', {
          hour: '2-digit',
          minute: '2-digit'
        })}
        {#if event.end_time}
          - {new Date(event.end_time).toLocaleTimeString('en-US', {
            hour: '2-digit',
            minute: '2-digit'
          })}
        {/if}
      </p>
    {/if}
    {#if event.description}
      <p class="text-sm text-gray-700 mt-2">{event.description}</p>
    {/if}
  </div>
{/if}

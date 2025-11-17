<script lang="ts">
    export let event: EventWithParticipants;

    function formatTime(time?: string) {
        return time ? new Date(time).toLocaleTimeString('en-US', {
            hour: '2-digit',
            minute: '2-digit'
        }) : null;
    }

    $: start = formatTime(event.start_time);
    $: end = formatTime(event.end_time);
</script>

<div
  class="p-4 bg-primary-hover bg-opacity-10 border-l-4 border-primary rounded-lg hover:bg-opacity-20 transition-colors">
    <h3 class="font-semibold text-lg mb-1">{event.title || 'Untitled Event'}</h3>
    {#if start}
        <p class="text-sm text-gray-600">
            {start}
            {#if end}
            - {end}
            {/if}
        </p>
    {/if}
    {#if event.description}
        <p class="text-sm text-gray-700 mt-2">{event.description}</p>
    {/if}
</div>
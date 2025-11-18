<script lang="ts">
    import type { EventWithParticipants } from "$lib/types";

    export let event: EventWithParticipants;
</script>

<div class="text-xs px-2 py-1 bg-primary-hover bg-opacity-10 text-primary rounded truncate">
    {event.title || 'Untitled Event'}
</div>
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { api } from '$lib/api';
  import type { EventWithParticipants } from '$lib/types';
  import EventCard from './atoms/event/EventCard.svelte';

  export let event: EventWithParticipants;

  const dispatch = createEventDispatcher();

  async function handleStatusChange(status: 'accepted' | 'declined' | 'maybe') {
    try {
      await api.updateParticipation(event.id, status);
      dispatch('refresh');
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to update status');
    }
  }

  async function handleDelete() {
    if (confirm('Are you sure you want to delete this event?')) {
      try {
        await api.deleteEvent(event.id);
        dispatch('refresh');
      } catch (err) {
        alert(err instanceof Error ? err.message : 'Failed to delete event');
      }
    }
  }
</script>

<EventCard 
  {event}
  on:delete={() => handleDelete()}
  on:accepted={() => handleStatusChange("accepted")}
  on:maybe={() => handleStatusChange("maybe")}
  on:declined={() => handleStatusChange("declined")}
/>
<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';

  export let event: EventWithParticipants;
</script>

<!-- Events you can see but weren't invited to (public, or a friend's
     friends-visible event) are outlined and muted rather than filled, so a
     friend's party doesn't read like something you owe an RSVP on. -->
<div
  class="text-xs px-2 py-1 rounded truncate {event.is_participant
    ? 'bg-primary-hover bg-opacity-10 text-primary'
    : 'border border-dashed border-line text-muted'}"
  title={event.is_participant
    ? undefined
    : "You're not invited to this one - it's just visible to you"}
>
  {event.title || 'Untitled Event'}
</div>

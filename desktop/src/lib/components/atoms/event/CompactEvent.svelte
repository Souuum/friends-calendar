<script lang="ts">
  import type { EventWithParticipants } from '$lib/types';
  import { statusOf } from '$lib/utils/eventStatus';

  export let event: EventWithParticipants;

  $: status = statusOf(event.my_status);
  $: declined = event.my_status === 'declined';
  $: count = event.participants?.length ?? 0;
</script>

<!--
  The mockup's month-grid chip: a 3px status rail, the title, and the
  headcount in mono - not a filled pill. The filled version made every event
  read as "going", since it was tinted regardless of status.

  Events you can see but weren't invited to stay visually distinct (the
  event-visibility-listing decision): the rail is muted and the title greys
  out, so a friend's party doesn't look like something you owe an answer on.
  That used to be a dashed outline, which the mockup's chip shape has no
  room for.
-->
<div
  class="flex w-full items-center gap-1.5 rounded-[5px] py-[3px] pr-1.5 text-left transition-colors hover:bg-subtle"
  title={event.is_participant
    ? undefined
    : "You're not invited to this one - it's just visible to you"}
>
  <span
    class="h-[15px] w-[3px] shrink-0 rounded-full"
    style="background:{event.is_participant ? status.bar : 'var(--color-line)'}"
  ></span>
  <span
    class="flex-1 truncate text-[12px] font-medium {declined || !event.is_participant
      ? 'text-muted'
      : 'text-ink'} {declined ? 'line-through' : ''}"
  >
    {event.title || 'Untitled Event'}
  </span>
  {#if count > 0}
    <span class="font-mono text-[10px] text-muted">{count}</span>
  {/if}
</div>

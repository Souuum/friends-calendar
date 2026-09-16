<script lang="ts">
  import Icon from '$lib/components/atoms/Icon.svelte';
  import type { EventWithParticipants } from '$lib/types';
  import EventCardParticipant from '$lib/components/atoms/event/EventCardParticipant.svelte';
  import { formatDate } from '$lib/utils/dateUtils';

  // Read-only: shows whether you (and everyone else) responded to a
  // calendar event - it doesn't let you change your RSVP from here, that
  // already exists via EventPeekPanel (opened from the calendar view).
  // Originally AnnouncementCard.svelte / the old event-RSVP /announcements
  // page; renamed when that page was replaced by a real Discord-message
  // feed (services::discord_feed) so "Announcement" stopped meaning this.
  // Still used by the friend-detail page's "Shared events" section.
  export let event: EventWithParticipants;

  function statusColor(status?: string) {
    switch (status) {
      case 'accepted':
        return 'bg-green-100 text-green-800';
      case 'declined':
        return 'bg-red-100 text-red-800';
      case 'maybe':
        return 'bg-yellow-100 text-yellow-800';
      default:
        return 'bg-gray-100 text-gray-800';
    }
  }

  // Icon separate from text, so it can be sized and themed. 'maybe' and
  // 'no response' intentionally have no icon: there is no glyph that reads
  // as "undecided" without inventing one, and the words already say it.
  function statusIcon(status?: string): 'accept' | 'decline' | undefined {
    if (status === 'accepted') return 'accept';
    if (status === 'declined') return 'decline';
    return undefined;
  }

  $: myStatusIcon = statusIcon(event.my_status);

  function statusLabel(status?: string) {
    switch (status) {
      case 'accepted':
        return 'You accepted';
      case 'declined':
        return 'You declined';
      case 'maybe':
        return 'You said maybe';
      default:
        return 'No response yet';
    }
  }
</script>

<div class="bg-white rounded-lg shadow p-5 space-y-3">
  <div class="flex justify-between items-start gap-3">
    <h3 class="font-bold text-lg text-gray-900">{event.title}</h3>
    <span
      class="shrink-0 inline-flex items-center gap-1 text-xs px-2 py-1 rounded {statusColor(
        event.my_status
      )}"
    >
      {#if myStatusIcon}
        <Icon name={myStatusIcon} size={12} />
      {/if}
      {statusLabel(event.my_status)}
    </span>
  </div>

  {#if event.description}
    <p class="text-gray-600 text-sm">{event.description}</p>
  {/if}

  <div class="space-y-1 text-sm text-gray-500">
    <div class="flex items-center gap-2">
      <Icon name="time" size={16} />
      <span>{formatDate(event.start_time)}</span>
    </div>
    {#if event.location}
      <div class="flex items-center gap-2">
        <Icon name="location" size={16} />
        <span>{event.location}</span>
      </div>
    {/if}
    {#if event.price}
      <div class="flex items-center gap-2">
        <Icon name="price" size={16} />
        <span>{event.price}</span>
      </div>
    {/if}
    {#if event.link}
      <!-- min-w-0 + truncate: a URL has no spaces, so it cannot wrap, and a
           flex child's default min-width:auto refuses to shrink below its
           content. Without both, a long link widened the whole page. -->
      <div class="flex items-center gap-2 min-w-0">
        <Icon name="link" size={16} class="shrink-0" />
        <a
          href={event.link}
          target="_blank"
          rel="noreferrer"
          class="text-discord-blurple hover:underline truncate"
        >
          {event.link}
        </a>
      </div>
    {/if}
  </div>

  <EventCardParticipant participants={event.participants} />
</div>

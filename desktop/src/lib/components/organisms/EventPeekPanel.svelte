<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { api } from '$lib/api';
  import type { EventWithParticipants } from '$lib/types';
  import { formatDate } from '$lib/utils/dateUtils';

  // The app's single event-detail surface: a side panel from `md:` up, a
  // bottom sheet below it. Replaced the old hover-tooltip (month) /
  // modal-on-click (week, day) interaction in mockup-calendar-redesign, and
  // superseded EventDetailsModal entirely in mockup-responsive-calendar -
  // that component was deleted rather than revived as the mobile sheet, since
  // it predated the mockup's STATUS palette and had no edit or
  // is_participant handling.
  export let event: EventWithParticipants | null = null;

  const dispatch = createEventDispatcher();

  let updating = false;
  let error = '';

  // Mirrors the mockup's STATUS object exactly (bar/bg/fg per status) -
  // "accepted" is tint/accent-text, never green.
  const STATUS: Record<string, { label: string; bar: string; bg: string; fg: string }> = {
    accepted: { label: 'Going', bar: '#5030e5', bg: '#eee8ff', fg: '#5030e5' },
    maybe: { label: 'Maybe', bar: '#fbb13c', bg: 'rgba(251,177,60,0.18)', fg: '#a9700f' },
    declined: { label: "Can't", bar: '#fb2c2c', bg: 'rgba(251,44,44,0.14)', fg: '#c01a1a' },
    pending: { label: 'No answer', bar: '#7c7c83', bg: '#f4f4f7', fg: '#5c5c61' }
  };

  function statusOf(status?: string) {
    return STATUS[status ?? 'pending'] ?? STATUS.pending;
  }

  // Two-step inline confirm rather than window.confirm(), to match how the
  // rest of this panel reports state (the `error` binding below) instead of
  // dropping a browser dialog on top of the app.
  let confirmingDelete = false;

  // Reset the confirm prompt when the selection changes, so it can't carry
  // over and delete a different event than the one it was armed for.
  $: if (event) {
    void event.id;
    confirmingDelete = false;
  }

  async function handleStatusChange(status: 'accepted' | 'declined' | 'maybe') {
    if (!event) return;
    try {
      updating = true;
      error = '';
      await api.updateParticipation(event.id, status);
      dispatch('refresh');
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to update status';
    } finally {
      updating = false;
    }
  }

  async function handleDelete() {
    if (!event) return;
    try {
      updating = true;
      error = '';
      await api.deleteEvent(event.id);
      confirmingDelete = false;
      dispatch('deleted');
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to delete event';
    } finally {
      updating = false;
    }
  }
</script>

<!-- One component, two placements. Below `md:` there's no room for a 296px
     side panel at 402px wide, so it docks to the bottom of the viewport as a
     sheet; from `md:` up it's the static side panel the mockup shows. Note
     this replaces the plan to revive EventDetailsModal for the mobile sheet:
     that component predates the mockup's status palette, uses
     confirm()/alert(), has no edit affordance and no is_participant
     handling, so reusing it would have reintroduced all four. -->
<aside
  class="bg-white border border-line p-[18px]
         fixed inset-x-0 bottom-0 z-40 max-h-[70vh] overflow-y-auto rounded-t-2xl shadow-[0_-14px_40px_rgba(0,0,0,0.18)] anim-sheet
         md:static md:z-auto md:max-h-none md:overflow-visible md:w-[296px] md:shrink-0 md:rounded-xl md:shadow-none md:animate-none
         {event ? '' : 'hidden md:block'}"
>
  {#if !event}
    <p class="text-sm text-gray-500">Select an event to see its details here.</p>
  {:else}
    <!-- Keyed on the event id so switching which event is selected replays
         the fade/grow entrance instead of silently patching the existing
         DOM in place (Svelte would otherwise just update text nodes) -
         matches the mockup's peek panel animating in fresh per selection. -->
    {#key event.id}
    <div class="anim-fade">
    <div
      class="h-1 rounded-full mb-3.5 anim-grow"
      style="background:{statusOf(event.my_status).bar}"
    ></div>

    <div class="flex items-center gap-2 mb-2">
      <span class="font-mono text-[11px] text-muted">{formatDate(event.start_time)}</span>
      {#if event.my_status}
        {@const s = statusOf(event.my_status)}
        <span
          class="rounded-full px-2.5 text-[11px] font-semibold py-1"
          style="background:{s.bg}; color:{s.fg}"
        >
          {s.label}
        </span>
      {/if}
    </div>

    <h2 class="text-[17px] font-bold tracking-[-0.01em] text-gray-900 mb-2.5">{event.title}</h2>
    <div class="flex flex-col gap-[5px] text-xs text-body mb-3.5">
      {#if event.location}
        <div>{event.location}</div>
      {/if}
      {#if event.price}
        <div>Cost per person: {event.price}</div>
      {/if}
    </div>

    {#if error}
      <p class="text-xs text-red-600 mb-2" role="alert">{error}</p>
    {/if}

    {#if !event.is_creator && !event.is_participant}
      <!-- Visible to you (public, or a friend's friends-visible event) but
           you're not on the guest list. No RSVP row: there's nothing to
           answer, and update_participation assumes a participant row. -->
      <div class="text-xs text-muted border border-line rounded-lg px-3 py-2 mb-4">
        You're not invited to this one — it's visible to you because
        {event.visibility === 'public' ? "it's public" : "you're friends with the organiser"}.
      </div>
    {:else if !event.is_creator}
      <!-- Going is always the solid primary CTA here - it does not
           reflect "currently selected", the mockup has no tint/fg
           selected-state on these three buttons (that pattern belongs to
           the status pill above and the filter chips, not this row). -->
      <div class="flex gap-1.5 mb-4">
        <button
          on:click={() => handleStatusChange('accepted')}
          disabled={updating}
          class="flex-1 py-2 rounded-lg text-xs font-semibold bg-primary text-white disabled:opacity-50"
        >
          Going
        </button>
        <button
          on:click={() => handleStatusChange('maybe')}
          disabled={updating}
          class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-white text-muted hover:bg-gray-50 disabled:opacity-50"
        >
          Maybe
        </button>
        <button
          on:click={() => handleStatusChange('declined')}
          disabled={updating}
          class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-white text-muted hover:border-red-600 hover:text-red-600 disabled:opacity-50"
        >
          Can't
        </button>
      </div>
    {:else}
      <!-- Creator's own event: "Edit"/"Nudge no-answers" per the mockup,
           not RSVP buttons. "Nudge no-answers" is still a placeholder -
           there is no endpoint that pings pending participants, and
           inventing one (a Discord DM path plus rate-limiting) is its own
           feature, not a side effect of wiring up Edit. -->
      <div class="flex gap-1.5 mb-4">
        <button
          on:click={() => dispatch('edit', event)}
          disabled={updating}
          class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-white text-muted hover:bg-gray-50 disabled:opacity-50"
        >
          Edit
        </button>
        <button
          disabled
          title="Not built yet - there's no endpoint to nudge pending participants"
          class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-white text-muted opacity-50 cursor-not-allowed"
        >
          Nudge no-answers
        </button>
      </div>

      <!-- Delete lives here because this panel is the only event detail UI
           in week and day view - the month-view hover tooltip
           (EventCardImpl) has had the only delete affordance, so there was
           no way to delete an event from the other two views at all. -->
      {#if confirmingDelete}
        <div class="flex gap-1.5 mb-4">
          <button
            on:click={handleDelete}
            disabled={updating}
            class="flex-1 py-2 rounded-lg text-xs font-semibold bg-red-600 text-white disabled:opacity-50"
          >
            {updating ? 'Deleting…' : 'Really delete'}
          </button>
          <button
            on:click={() => (confirmingDelete = false)}
            disabled={updating}
            class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-white text-muted hover:bg-gray-50 disabled:opacity-50"
          >
            Cancel
          </button>
        </div>
      {:else}
        <button
          on:click={() => (confirmingDelete = true)}
          disabled={updating}
          class="w-full py-2 mb-4 rounded-lg text-xs font-semibold border border-line bg-white text-muted hover:border-red-600 hover:text-red-600 disabled:opacity-50"
        >
          Delete event
        </button>
      {/if}
    {/if}

    <div class="font-mono text-[10px] tracking-widest uppercase text-muted mb-2">
      {event.participants.length} invited
    </div>
    <div class="flex flex-col gap-2">
      {#each event.participants as participant}
        {@const s = statusOf(participant.status)}
        <div class="flex items-center gap-2">
          {#if participant.avatar_url}
            <img src={participant.avatar_url} alt="" class="w-7 h-7 rounded-full" />
          {:else}
            <div class="w-7 h-7 rounded-full bg-gray-300"></div>
          {/if}
          <span class="flex-1 text-sm text-gray-800 truncate">{participant.username}</span>
          <span
            class="text-[11px] px-2.5 py-1 rounded-full font-semibold"
            style="background:{s.bg}; color:{s.fg}"
          >
            {s.label}
          </span>
        </div>
      {/each}
    </div>
    </div>
    {/key}
  {/if}
</aside>

<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { api } from '$lib/api';
  import type { EventWithParticipants } from '$lib/types';
  import { formatDate } from '$lib/utils/dateUtils';

  // Persistent side panel replacing the old hover-tooltip (month) /
  // modal-on-click (week, day) interaction - see
  // .claude/skills/mockup-calendar-redesign/SKILL.md. The RSVP call itself
  // mirrors EventDetailsModal.svelte's handleStatusChange, but the colors
  // below are this screen's own (the mockup's STATUS map), not copied from
  // that older component - see the skill for why those two don't share a
  // palette even though they share behavior.
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
</script>

<aside class="w-[296px] shrink-0 bg-white border border-line rounded-xl p-[18px]">
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

    {#if !event.is_creator}
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
           not RSVP buttons. Non-functional placeholders for now - there's
           no edit-event flow or no-answer-nudge endpoint yet, and adding
           those is out of scope for a visual-parity fix; shown disabled
           rather than omitted so the layout matches the mockup. -->
      <div class="flex gap-1.5 mb-4">
        <button
          disabled
          class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-white text-muted opacity-50 cursor-not-allowed"
        >
          Edit
        </button>
        <button
          disabled
          class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-white text-muted opacity-50 cursor-not-allowed"
        >
          Nudge no-answers
        </button>
      </div>
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

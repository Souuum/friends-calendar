<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { dismissable } from '$lib/actions/dismissable';
  // Shared with the month-grid chip so the two can't disagree - see the
  // module doc.
  import { statusOf } from '$lib/utils/eventStatus';
  import { api } from '$lib/api';
  import type { EventWithParticipants } from '$lib/types';
  import { formatDate } from '$lib/utils/dateUtils';

  // The app's single event-detail surface: a side panel from `lg:` up, a
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

  let nudging = false;
  let nudgeMessage = '';

  // Derived from the participants already on the event - no extra request,
  // and it can't disagree with the list rendered below.
  $: pendingCount = (event?.participants ?? []).filter((p) => p.status === 'pending').length;

  // Reset when the selection changes, so a message about one event can't
  // sit above another - the same reason `confirmingDelete` resets below.
  $: if (event) {
    void event.id;
    nudgeMessage = '';
  }

  async function handleNudge() {
    if (!event) return;
    try {
      nudging = true;
      error = '';
      const report = await api.nudgeNoAnswers(event.id);
      nudgeMessage =
        report.nudged === 1 ? 'Reminded 1 person.' : `Reminded ${report.nudged} people.`;
      if (report.discord_failed) {
        // Said plainly rather than swallowed: the in-app reminders did go
        // out, so this is a partial success, not a failure.
        nudgeMessage += " Couldn't post in the Discord thread.";
      }
    } catch (err) {
      // The rate limit comes back as a message naming when the next one is
      // allowed, so show it rather than a generic failure.
      error = err instanceof Error ? err.message : 'Could not nudge';
    } finally {
      nudging = false;
    }
  }

  /**
   * Below `lg:` this is a sheet floating over the calendar, and it shipped
   * with **no way to dismiss it at all** - no close control, no backdrop,
   * and nothing listening for a tap outside. Selecting a different event
   * was the only thing that changed it. From `lg:` up it's a static column
   * with an explicit "select an event" empty state, so it's *meant* to
   * persist there and deliberately keeps doing so.
   *
   * Tracked in JS rather than with a `lg:hidden` backdrop, because the
   * difference isn't only visual: the backdrop and its history entry must
   * not exist at all on desktop, where nothing is being covered up.
   */
  let isSheet = false;

  onMount(() => {
    // 1023.98 rather than 1023: `lg:` is min-width 1024px, and a fractional
    // viewport width (browser zoom, some devices) would otherwise fall in
    // the gap between the two and match neither.
    //
    // ⚠️ `lg:`, not `md:`. At 768 the 216px rail plus this 296px column
    // leaves ~192px for seven day cells - 27px each, which truncated every
    // event chip's title to zero width. The panel only docks once there is
    // room for both.
    const sheetWidth = window.matchMedia('(max-width: 1023.98px)');
    const sync = () => (isSheet = sheetWidth.matches);
    sync();
    sheetWidth.addEventListener('change', sync);
    return () => sheetWidth.removeEventListener('change', sync);
  });

  function close() {
    dispatch('close');
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

<!-- One component, two placements. Below `lg:` there isn't room for a 296px
     side panel beside a seven-column grid, so it docks to the bottom of the
     viewport as a sheet; from `lg:` up it's the static side panel the mockup
     shows. Note
     this replaces the plan to revive EventDetailsModal for the mobile sheet:
     that component predates the mockup's status palette, uses
     confirm()/alert(), has no edit affordance and no is_participant
     handling, so reusing it would have reintroduced all four. -->
<!-- Transparent on purpose: the sheet is docked over the calendar rather
     than presented as a modal, and dimming everything would change that
     design. This exists to catch the tap, not to be seen. It sits below the
     sheet (z-30) and, being earlier in the DOM than `Frame`'s equally-ranked
     tab bar, below that too - so the tabs still navigate while it's open. -->
{#if event && isSheet}
  <div
    class="fixed inset-0 z-30 lg:hidden"
    use:dismissable={close}
    role="presentation"
    aria-hidden="true"
  ></div>
{/if}

<aside
  data-testid="event-peek"
  class="bg-surface border border-line p-[18px]
         fixed inset-x-0 bottom-0 z-40 max-h-[70vh] overflow-y-auto rounded-t-2xl shadow-[0_-14px_40px_rgba(0,0,0,0.18)] anim-sheet
         lg:static lg:z-auto lg:max-h-none lg:overflow-visible lg:w-[296px] lg:shrink-0 lg:rounded-xl lg:shadow-none lg:animate-none
         {event ? '' : 'hidden lg:block'}"
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
          <!-- Sheet only: from `lg:` up the panel is a column that is
               supposed to stay, so there is nothing to close. -->
          {#if isSheet}
            <button
              type="button"
              on:click={close}
              class="-mr-2 ml-auto flex min-h-[44px] min-w-[44px] shrink-0 items-center justify-center rounded-lg text-2xl leading-none text-gray-400 hover:bg-gray-100 hover:text-gray-600 lg:hidden"
              aria-label="Close">×</button
            >
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
              class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-surface text-muted hover:bg-gray-50 disabled:opacity-50"
            >
              Maybe
            </button>
            <button
              on:click={() => handleStatusChange('declined')}
              disabled={updating}
              class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-surface text-muted hover:border-red-600 hover:text-red-600 disabled:opacity-50"
            >
              Can't
            </button>
          </div>
        {:else}
          <!-- Creator's own event: "Edit"/"Nudge no-answers" per the mockup,
           not RSVP buttons. -->
          <div class="flex gap-1.5 mb-4">
            <button
              on:click={() => dispatch('edit', event)}
              disabled={updating}
              class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-surface text-muted hover:bg-gray-50 disabled:opacity-50"
            >
              Edit
            </button>
            <!-- Disabled when there is nobody to chase, with the reason on
                 the button - rather than enabled and silently doing nothing,
                 which is what it did for the whole time it was a
                 placeholder. -->
            <button
              on:click={handleNudge}
              disabled={nudging || pendingCount === 0}
              title={pendingCount === 0
                ? 'Everyone has answered'
                : pendingCount === 1
                  ? "Remind 1 person who hasn't answered"
                  : `Remind ${pendingCount} people who haven't answered`}
              class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-surface text-muted hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {nudging ? 'Nudging…' : 'Nudge no-answers'}
            </button>
          </div>

          {#if nudgeMessage}
            <p class="mb-3 text-xs text-primary" role="status">{nudgeMessage}</p>
          {/if}

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
                class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-surface text-muted hover:bg-gray-50 disabled:opacity-50"
              >
                Cancel
              </button>
            </div>
          {:else}
            <button
              on:click={() => (confirmingDelete = true)}
              disabled={updating}
              class="w-full py-2 mb-4 rounded-lg text-xs font-semibold border border-line bg-surface text-muted hover:border-red-600 hover:text-red-600 disabled:opacity-50"
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

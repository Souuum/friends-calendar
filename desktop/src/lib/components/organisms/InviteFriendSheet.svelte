<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { api } from '$lib/api';
  import { dismissable } from '$lib/actions/dismissable';
  import type { BestSlot, CalendarEvent, FriendInfo } from '$lib/types';
  import { formatDate } from '$lib/utils/dateUtils';

  /**
   * "Invite {name} to something."
   *
   * Two routes out of one control, because an app that could only add
   * someone to an *existing* event would be useless exactly when you
   * haven't made it yet:
   *
   *  - one of your upcoming events they aren't on, or
   *  - a new event started with them already in it.
   *
   * The event list comes from `GET /api/events/invitable`, which is scoped
   * to events **you created** - that is what `invite_participants` enforces,
   * so offering anything else would be a choice that silently does nothing.
   */
  export let friend: FriendInfo;

  const dispatch = createEventDispatcher<{
    close: void;
    invited: { event: CalendarEvent };
    createWith: { start: Date | null };
  }>();

  let events: CalendarEvent[] = [];
  let loading = true;
  let error = '';
  let inviting = '';

  /**
   * A time you are *both* free, for the "new event" route. Best-effort: the
   * form opens either way, just without a suggestion.
   */
  let bestSlot: BestSlot | null = null;

  onMount(async () => {
    try {
      events = await api.getInvitableEvents(friend.user_id);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load your events';
    } finally {
      loading = false;
    }

    try {
      const from = new Date();
      const to = new Date(from.getTime() + 7 * 24 * 60 * 60 * 1000);
      [bestSlot] = await api.getBestSlots(from, to, 120, friend.user_id);
    } catch {
      bestSlot = null;
    }
  });

  async function inviteTo(event: CalendarEvent) {
    try {
      inviting = event.id;
      error = '';
      // The existing endpoint, not a second invite path.
      await api.inviteParticipants(event.id, [friend.user_id]);
      dispatch('invited', { event });
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not invite';
    } finally {
      inviting = '';
    }
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-end justify-center overflow-y-auto bg-black bg-opacity-50 p-0 anim-scrim sm:items-start sm:p-4"
  use:dismissable={() => dispatch('close')}
  role="presentation"
>
  <div
    class="bg-surface my-auto w-full max-w-lg rounded-t-2xl shadow-2xl anim-sheet sm:rounded-2xl sm:anim-pop"
    role="dialog"
    aria-modal="true"
    aria-label={`Invite ${friend.username}`}
  >
    <div class="p-[18px]">
      <div class="mb-3 flex items-start justify-between gap-3">
        <h2 class="m-0 text-[17px] font-bold">Invite {friend.username}</h2>
        <button
          type="button"
          on:click={() => dispatch('close')}
          class="-mr-2 -mt-1 flex min-h-[44px] min-w-[44px] shrink-0 items-center justify-center rounded-lg text-2xl leading-none text-gray-400 hover:bg-gray-100 hover:text-gray-600"
          aria-label="Close">×</button
        >
      </div>

      {#if error}
        <p class="mb-3 text-[13px] text-red-600" role="alert">{error}</p>
      {/if}

      {#if loading}
        <p class="text-[13px] text-muted">Loading your events…</p>
      {:else if events.length > 0}
        <p class="m-0 mb-2 font-mono text-[10px] uppercase tracking-[0.1em] text-muted">
          Your events
        </p>
        <div class="mb-4 flex flex-col gap-1.5">
          {#each events as event (event.id)}
            <button
              type="button"
              on:click={() => inviteTo(event)}
              disabled={inviting !== ''}
              class="flex w-full items-center gap-2.5 rounded-[11px] border border-line bg-surface px-3 py-2.5 text-left hover:bg-subtle disabled:opacity-50"
            >
              <span class="min-w-0 flex-1">
                <span class="block truncate text-[14px] font-semibold">{event.title}</span>
                <span class="block font-mono text-[11px] text-muted">
                  {formatDate(event.start_time)}
                </span>
              </span>
              <span class="shrink-0 text-[12px] font-semibold text-primary">
                {inviting === event.id ? 'Inviting…' : 'Invite'}
              </span>
            </button>
          {/each}
        </div>
      {:else}
        <p class="m-0 mb-4 text-[13px] text-muted">
          You have no upcoming events {friend.username} isn't already on.
        </p>
      {/if}

      <button
        type="button"
        on:click={() =>
          dispatch('createWith', { start: bestSlot ? new Date(bestSlot.start) : null })}
        class="w-full rounded-[9px] bg-primary px-3.5 py-[11px] text-[13px] font-semibold text-white hover:bg-primary-active"
      >
        New event with {friend.username}
      </button>
      {#if bestSlot}
        <!-- The payoff from availability-best-overlap: "invite them" becomes
             "invite them to a time you're both free". -->
        <p class="m-0 mt-2 text-center text-[12px] text-muted">
          You're both free
          {new Date(bestSlot.start).toLocaleString(undefined, {
            weekday: 'long',
            hour: '2-digit',
            minute: '2-digit'
          })}
        </p>
      {/if}
    </div>
  </div>
</div>

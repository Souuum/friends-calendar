<script lang="ts">
  import Icon from '$lib/components/atoms/Icon.svelte';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import EventRsvpCard from '$lib/components/molecules/EventRsvpCard.svelte';
  import { dateUtils } from '$lib/utils/dateUtils';
  import type {
    CalendarEvent,
    DayAvailability,
    EventWithParticipants,
    FriendInfo
  } from '$lib/types';
  import InviteFriendSheet from '$lib/components/organisms/InviteFriendSheet.svelte';
  import CreateEventModal from '$lib/components/CreateEventModal.svelte';

  // No dedicated GET /api/friends/:id endpoint - the friends list already
  // has everything the header needs, and "shared events" is a filter over
  // GET /api/events. Deliberately omits the mockup's "Mutual servers" card
  // (this app links exactly one Discord server, so every friend's page
  // would show the identical card - not informational).
  // See mockup-friends-directory skill for the full rationale.

  let friend: FriendInfo | undefined;
  let sharedEvents: EventWithParticipants[] = [];
  let loading = true;
  let error = '';

  let availability: DayAvailability[] = [];

  $: friendId = $page.params.id;

  const WEEKDAY_LABELS = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];

  async function load(id: string) {
    try {
      loading = true;
      error = '';
      const [friends, events] = await Promise.all([
        api.getFriends(),
        api.getEvents({ include_declined: true })
      ]);
      friend = friends.find((f) => f.user_id === id);
      sharedEvents = events
        // `is_participant` matters since GET /api/events started returning
        // events you can merely see (public, or a friend's friends-visible
        // event). Without it, "Shared events" would list events only *they*
        // are in, which is the opposite of shared.
        .filter((e) => e.is_participant && e.participants.some((p) => p.user_id === id))
        .sort((a, b) => new Date(b.start_time).getTime() - new Date(a.start_time).getTime());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load friend';
    } finally {
      loading = false;
    }

    // Best-effort, separate from the main load - the friend page is still
    // useful without it (e.g. the availability call requires being
    // actual friends, which we already know is true if we got here).
    try {
      const weekStart = dateUtils.startOfWeek(new Date()).toISOString();
      availability = await api.getWeekAvailability(id, weekStart);
    } catch {
      availability = [];
    }
  }

  function fillClass(freeCount: number): string {
    if (freeCount >= 2) return 'bg-green-300';
    if (freeCount === 1) return 'bg-yellow-100';
    return 'bg-gray-200';
  }

  onMount(() => {
    if (friendId) load(friendId);
  });

  // The invite sheet. One nullable/boolean pair kept deliberately small:
  // `inviting` is whether the sheet is open, `invitedTo` the title of the
  // event we just added them to, for the confirmation line.
  let inviting = false;
  let invitedTo = '';

  // Opened with the friend preselected, and - when the group's availability
  // could be computed - on a time you're both free.
  let creatingWith: Date | null = null;
  let showCreateModal = false;

  function handleInvited(event: CustomEvent<{ event: CalendarEvent }>) {
    inviting = false;
    invitedTo = event.detail.event.title;
  }

  function handleCreateWith(event: CustomEvent<{ start: Date | null }>) {
    inviting = false;
    creatingWith = event.detail.start;
    showCreateModal = true;
  }

  function handleCreated() {
    showCreateModal = false;
    creatingWith = null;
    // Re-read so the new event shows under "Shared events" straight away.
    if (friend) load(friend.user_id);
  }
</script>

<svelte:head>
  <title>{friend ? friend.username : 'Friend'} - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl anim-fade-up">
    <button
      class="inline-flex items-center gap-1 text-sm text-discord-blurple hover:underline mb-4 bg-transparent border-none cursor-pointer p-0"
      on:click={() => goto('/friends')}
    >
      <Icon name="back" size={14} /> Back to friends
    </button>

    {#if error}
      <p class="text-sm text-red-600" role="alert">{error}</p>
    {:else if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if !friend}
      <p class="text-sm text-gray-500">Friend not found.</p>
    {:else}
      <div class="flex items-center gap-4 mb-6">
        {#if friend.avatar_url}
          <img src={friend.avatar_url} alt="" class="w-16 h-16 rounded-full" />
        {:else}
          <div class="w-16 h-16 rounded-full bg-gray-300"></div>
        {/if}
        <h1 class="text-2xl font-semibold m-0">{friend.username}</h1>

        <!-- The screen's primary action in both mockups, and it had none at
             all: grep this file for "invite" before this landed and there
             were zero hits. -->
        <button
          type="button"
          on:click={() => (inviting = true)}
          class="ml-auto shrink-0 rounded-[9px] bg-primary px-3.5 py-[9px] text-[13px] font-semibold text-white hover:bg-primary-active"
        >
          Invite
        </button>
      </div>

      {#if invitedTo}
        <p class="mb-4 rounded-lg bg-tint px-3 py-2 text-[13px] text-primary" role="status">
          Invited {friend.username} to "{invitedTo}".
        </p>
      {/if}

      {#if availability.length === 7}
        <div class="bg-surface border border-gray-200 rounded-xl p-4 mb-6">
          <h2 class="text-sm font-semibold mb-1">Free this week</h2>
          <p class="text-xs text-gray-500 mb-3">Overlap with your calendar</p>
          <div class="flex gap-1.5">
            {#each availability as day, i (day.date)}
              <div class="flex-1 text-center">
                <div class="font-mono text-[10px] text-gray-400 mb-1">{WEEKDAY_LABELS[i]}</div>
                <div
                  class="h-12 rounded {fillClass(day.free_user_ids.length)}"
                  title={`${day.free_user_ids.length} of 2 free`}
                ></div>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <h2 class="text-lg font-semibold mb-3">Shared events</h2>
      {#if sharedEvents.length === 0}
        <p class="text-sm text-gray-500">No shared events yet.</p>
      {:else}
        <div class="space-y-3">
          {#each sharedEvents as event (event.id)}
            <EventRsvpCard {event} />
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  {#if inviting && friend}
    <InviteFriendSheet
      {friend}
      on:close={() => (inviting = false)}
      on:invited={handleInvited}
      on:createWith={handleCreateWith}
    />
  {/if}

  {#if showCreateModal && friend}
    <CreateEventModal
      event={null}
      initialDate={creatingWith}
      initialParticipantIds={[friend.user_id]}
      on:close={() => (showCreateModal = false)}
      on:saved={handleCreated}
    />
  {/if}
</Frame>

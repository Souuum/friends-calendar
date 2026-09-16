<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { unreadNotificationCount } from '$lib/stores';
  import Frame from '$lib/components/templates/Frame.svelte';
  import { groupByRecency } from '$lib/utils/notificationUtils';
  import type { NotificationInfo, Status } from '$lib/types';

  let notifications: NotificationInfo[] = [];
  let loading = true;
  let error = '';

  // Per-notification RSVP state, keyed by notification id. Kept out of the
  // NotificationInfo objects themselves so a refetch can't clobber it, and
  // so one card's failure can't blank the rest of the list.
  let rsvpPending = new Set<string>();
  let rsvpErrors: Record<string, string> = {};

  async function load() {
    try {
      loading = true;
      error = '';
      notifications = await api.getNotifications();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load notifications';
    } finally {
      loading = false;
    }
  }

  async function markRead(id: string) {
    // Optimistic - flip locally first, no full refetch just to toggle one row.
    notifications = notifications.map((n) => (n.id === id ? { ...n, read: true } : n));
    try {
      await api.markNotificationRead(id);
      unreadNotificationCount.set(await api.getUnreadNotificationCount());
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to mark notification read';
    }
  }

  async function markAllRead() {
    const previous = notifications;
    notifications = notifications.map((n) => ({ ...n, read: true }));
    try {
      await api.markAllNotificationsRead();
      unreadNotificationCount.set(0);
    } catch (err) {
      notifications = previous;
      error = err instanceof Error ? err.message : 'Failed to mark all notifications read';
    }
  }

  // Answering an invite from here also marks it read: acting on a
  // notification is acknowledgement, and making people tap twice for that
  // would be busywork.
  async function respond(notification: NotificationInfo, status: Status) {
    if (!notification.event_id) return;

    rsvpPending = new Set(rsvpPending).add(notification.id);
    rsvpErrors = { ...rsvpErrors, [notification.id]: '' };

    try {
      await api.updateParticipation(notification.event_id, status as 'accepted' | 'declined' | 'maybe');
      await markRead(notification.id);
    } catch (err) {
      // Scoped to this card. The event may have been deleted, or you may
      // have been removed from it, since the notification was written -
      // that must not take down the whole list.
      rsvpErrors = {
        ...rsvpErrors,
        [notification.id]: err instanceof Error ? err.message : 'Failed to update your answer'
      };
    } finally {
      const next = new Set(rsvpPending);
      next.delete(notification.id);
      rsvpPending = next;
    }
  }

  // Only invites you can still act on: a notification with no event_id
  // (older rows, friend requests) has nothing to RSVP to.
  function canRsvp(n: NotificationInfo): boolean {
    return n.kind === 'event_invite' && !!n.event_id;
  }

  function formatTime(iso: string): string {
    return new Date(iso).toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  $: unreadInList = notifications.filter((n) => !n.read).length;
  $: groups = groupByRecency(notifications);

  onMount(load);
</script>

<svelte:head>
  <title>Notifications - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl mx-auto py-6 px-4 anim-fade-up">
    <div class="flex items-end gap-4 flex-wrap mb-5">
      <div>
        <h1 class="text-2xl font-semibold m-0">Notifications</h1>
        <p class="text-sm text-gray-500 m-0 mt-1">{unreadInList} unread</p>
      </div>
      {#if notifications.length > 0}
        <button
          on:click={markAllRead}
          class="ml-auto px-3 py-1.5 border border-gray-300 rounded-lg text-sm font-medium hover:bg-gray-50"
        >
          Mark all read
        </button>
      {/if}
    </div>

    {#if error}
      <p class="text-sm text-red-600" role="alert">{error}</p>
    {:else if loading}
      <p class="text-sm text-gray-500">Loading…</p>
    {:else if notifications.length === 0}
      <p class="text-sm text-gray-500">Nothing yet.</p>
    {:else}
      {#each groups as group (group.label)}
        <h2 class="font-mono text-[10px] tracking-widest uppercase text-muted mt-5 mb-2">
          {group.label}
        </h2>
        <div
          class="bg-white border border-gray-200 rounded-xl overflow-hidden divide-y divide-gray-100"
        >
          {#each group.notifications as notification, i (notification.id)}
            <div
              style="animation-delay: {i * 45}ms"
              class="anim-slide-left {notification.read ? 'bg-white' : 'bg-indigo-50'}"
            >
              <button
                on:click={() => markRead(notification.id)}
                class="w-full text-left flex items-center gap-3 p-4"
              >
                {#if notification.actor_avatar_url}
                  <img
                    src={notification.actor_avatar_url}
                    alt=""
                    class="w-9 h-9 rounded-full flex-shrink-0"
                  />
                {:else}
                  <div class="w-9 h-9 rounded-full bg-gray-300 flex-shrink-0"></div>
                {/if}
                <div class="flex-1 min-w-0">
                  <p class="text-sm m-0">{notification.message}</p>
                  <p class="text-xs text-gray-500 m-0 mt-1 font-mono">
                    {formatTime(notification.created_at)}
                  </p>
                </div>
                {#if !notification.read}
                  <span class="w-2 h-2 rounded-full bg-red-500 flex-shrink-0 anim-pulse-dot"></span>
                {/if}
              </button>

              {#if canRsvp(notification)}
                <!-- Two flexible buttons plus a narrower "Can't", per the
                     mockup - three equal-width buttons don't fit at 402px. -->
                <div class="flex gap-1.5 px-4 pb-3 -mt-1">
                  <button
                    on:click={() => respond(notification, 'accepted')}
                    disabled={rsvpPending.has(notification.id)}
                    class="flex-1 py-2 rounded-lg text-xs font-semibold bg-primary text-white disabled:opacity-50"
                  >
                    Going
                  </button>
                  <button
                    on:click={() => respond(notification, 'maybe')}
                    disabled={rsvpPending.has(notification.id)}
                    class="flex-1 py-2 rounded-lg text-xs font-semibold border border-line bg-white text-muted hover:bg-gray-50 disabled:opacity-50"
                  >
                    Maybe
                  </button>
                  <button
                    on:click={() => respond(notification, 'declined')}
                    disabled={rsvpPending.has(notification.id)}
                    class="shrink-0 px-3 py-2 rounded-lg text-xs font-semibold border border-line bg-white text-muted hover:border-red-600 hover:text-red-600 disabled:opacity-50"
                  >
                    Can't
                  </button>
                </div>
                {#if rsvpErrors[notification.id]}
                  <p class="text-xs text-red-600 px-4 pb-3 m-0" role="alert">
                    {rsvpErrors[notification.id]}
                  </p>
                {/if}
              {/if}
            </div>
          {/each}
        </div>
      {/each}
    {/if}
  </div>
</Frame>

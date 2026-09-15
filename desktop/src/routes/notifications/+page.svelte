<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { unreadNotificationCount } from '$lib/stores';
  import Frame from '$lib/components/templates/Frame.svelte';
  import type { NotificationInfo } from '$lib/types';

  let notifications: NotificationInfo[] = [];
  let loading = true;
  let error = '';

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

  function formatTime(iso: string): string {
    return new Date(iso).toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  $: unreadInList = notifications.filter((n) => !n.read).length;

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
      <div class="bg-white border border-gray-200 rounded-xl overflow-hidden divide-y divide-gray-100">
        {#each notifications as notification, i (notification.id)}
          <button
            on:click={() => markRead(notification.id)}
            style="animation-delay: {i * 45}ms"
            class="w-full text-left flex items-center gap-3 p-4 anim-slide-left {notification.read
              ? 'bg-white'
              : 'bg-indigo-50'}"
          >
            {#if notification.actor_avatar_url}
              <img src={notification.actor_avatar_url} alt="" class="w-9 h-9 rounded-full flex-shrink-0" />
            {:else}
              <div class="w-9 h-9 rounded-full bg-gray-300 flex-shrink-0"></div>
            {/if}
            <div class="flex-1 min-w-0">
              <p class="text-sm m-0">{notification.message}</p>
              <p class="text-xs text-gray-500 m-0 mt-1 font-mono">{formatTime(notification.created_at)}</p>
            </div>
            {#if !notification.read}
              <span class="w-2 h-2 rounded-full bg-red-500 flex-shrink-0 anim-pulse-dot"></span>
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</Frame>

<script lang="ts">
  import type { FriendInfo } from '$lib/types';
  import Avatar from '$lib/components/atoms/Avatar.svelte';

  // Presentational, like EventList.svelte — data comes in via props, this
  // component doesn't call `api` itself. Not wired into the app yet; the
  // container that would own loading/fetching (mirroring CalendarView.svelte
  // for events) hasn't been built.
  export let friends: FriendInfo[] = [];
  export let syncing = false;
  export let error = '';
  export let onSync: (() => void) | undefined = undefined;
</script>

<div class="space-y-2">
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold">Friends</h2>
    {#if onSync}
      <button
        on:click={onSync}
        disabled={syncing}
        class="px-3 py-1.5 rounded-lg font-medium text-sm transition bg-discord-blurple hover:bg-blue-600 text-white disabled:opacity-50"
      >
        {syncing ? 'Syncing…' : 'Sync friends'}
      </button>
    {/if}
  </div>

  {#if error}
    <p class="text-sm text-red-600" role="alert">{error}</p>
  {:else if friends.length === 0}
    <p class="text-sm text-gray-500">No friends synced yet.</p>
  {:else}
    <ul class="space-y-1">
      {#each friends as friend (friend.user_id)}
        <li class="flex items-center gap-2">
          <Avatar src={friend.avatar_url ?? ''} size={32} />
          <span class="text-sm font-medium">{friend.username}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

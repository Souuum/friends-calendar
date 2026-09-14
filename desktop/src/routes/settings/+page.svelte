<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import LinkedServerCard from '$lib/components/molecules/LinkedServerCard.svelte';
  import FriendsList from '$lib/components/molecules/FriendsList.svelte';
  import type { LinkedServerInfo, FriendInfo } from '$lib/types';

  let server: LinkedServerInfo | null = null;
  let serverError = '';
  let serverLoading = true;

  let friends: FriendInfo[] = [];
  let friendsError = '';
  let friendsLoading = true;
  let syncing = false;

  async function loadServer() {
    try {
      serverLoading = true;
      serverError = '';
      server = await api.getLinkedServer();
    } catch (err) {
      serverError = err instanceof Error ? err.message : 'Failed to load linked server';
    } finally {
      serverLoading = false;
    }
  }

  async function loadFriends() {
    try {
      friendsLoading = true;
      friendsError = '';
      friends = await api.getFriends();
    } catch (err) {
      friendsError = err instanceof Error ? err.message : 'Failed to load friends';
    } finally {
      friendsLoading = false;
    }
  }

  async function handleSync() {
    try {
      syncing = true;
      friendsError = '';
      const result = await api.syncFriends();
      friends = result.friends;
    } catch (err) {
      friendsError = err instanceof Error ? err.message : 'Failed to sync friends';
    } finally {
      syncing = false;
    }
  }

  onMount(() => {
    loadServer();
    loadFriends();
  });
</script>

<svelte:head>
  <title>Settings - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl mx-auto py-6 px-4 space-y-6">
    <button class="text-sm text-discord-blurple hover:underline" on:click={() => goto('/')}>
      ← Back to calendar
    </button>

    <section>
      <h1 class="text-xl font-semibold mb-3">Linked Discord server</h1>
      {#if serverError}
        <p class="text-sm text-red-600" role="alert">{serverError}</p>
      {:else if serverLoading}
        <p class="text-sm text-gray-500">Loading…</p>
      {:else if server}
        <LinkedServerCard {server} />
      {/if}
    </section>

    <section>
      {#if friendsLoading}
        <p class="text-sm text-gray-500">Loading friends…</p>
      {:else}
        <FriendsList {friends} error={friendsError} {syncing} onSync={handleSync} />
      {/if}
    </section>
  </div>
</Frame>

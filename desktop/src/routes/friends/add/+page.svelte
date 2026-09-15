<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import Frame from '$lib/components/templates/Frame.svelte';
  import type { FriendRequestInfo } from '$lib/types';

  // "Username", not the mockup's "Discord tag" - the backend looks people
  // up by their app username (services::friend_requests::send_request),
  // there's no Discord-tag lookup implemented. Matching the copy to what
  // this actually does rather than cloning the mockup's field label.
  let username = '';
  let sendError = '';
  let sendStatus = '';
  let sending = false;

  let requests: FriendRequestInfo[] = [];
  let requestsError = '';
  let requestsLoading = true;

  let missingCount: number | null = null;
  let inviteSent = false;
  let inviteError = '';

  async function loadRequests() {
    try {
      requestsLoading = true;
      requestsError = '';
      requests = await api.listFriendRequests();
    } catch (err) {
      requestsError = err instanceof Error ? err.message : 'Failed to load requests';
    } finally {
      requestsLoading = false;
    }
  }

  async function loadMissingCount() {
    try {
      missingCount = await api.getMissingMembersCount();
    } catch {
      // Optional section (needs a linked Discord server) - don't block the
      // rest of the page over it.
    }
  }

  async function handleSend() {
    const target = username.trim();
    if (!target) return;
    try {
      sending = true;
      sendError = '';
      sendStatus = '';
      const result = await api.sendFriendRequest(target);
      sendStatus = result.status === 'auto_accepted' ? `You and ${target} are now friends!` : 'Request sent.';
      username = '';
    } catch (err) {
      sendError = err instanceof Error ? err.message : 'Failed to send request';
    } finally {
      sending = false;
    }
  }

  async function respond(id: string, accept: boolean) {
    try {
      if (accept) {
        await api.acceptFriendRequest(id);
      } else {
        await api.declineFriendRequest(id);
      }
      requests = requests.filter((r) => r.id !== id);
    } catch (err) {
      requestsError = err instanceof Error ? err.message : 'Failed to respond to request';
    }
  }

  async function handlePostInvite() {
    try {
      inviteError = '';
      await api.postGuildInvite();
      inviteSent = true;
    } catch (err) {
      inviteError = err instanceof Error ? err.message : 'Failed to post invite';
    }
  }

  onMount(() => {
    loadRequests();
    loadMissingCount();
  });
</script>

<svelte:head>
  <title>Add friends - Friends Calendar</title>
</svelte:head>

<Frame>
  <div class="max-w-2xl mx-auto py-6 px-4 space-y-5 anim-fade-up">
    <button
      class="text-sm text-discord-blurple hover:underline bg-transparent border-none cursor-pointer p-0"
      on:click={() => goto('/friends')}
    >
      ← Back to friends
    </button>

    <div>
      <h1 class="text-2xl font-semibold m-0">Add friends</h1>
      <p class="text-sm text-gray-500 m-0 mt-1">Send a request by their username.</p>
    </div>

    <section class="bg-white border border-gray-200 rounded-xl p-5">
      <label for="username" class="block text-sm font-semibold mb-2">Username</label>
      <form on:submit|preventDefault={handleSend} class="flex gap-2 flex-wrap">
        <input
          id="username"
          type="text"
          bind:value={username}
          placeholder="username"
          class="flex-1 min-w-[180px] px-3 py-2 border border-gray-300 rounded-lg text-sm focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
        />
        <button
          type="submit"
          disabled={sending || !username.trim()}
          class="px-4 py-2 bg-primary text-white rounded-lg text-sm font-semibold disabled:opacity-50"
        >
          {sending ? 'Sending…' : 'Send request'}
        </button>
      </form>
      {#if sendError}
        <p class="text-sm text-red-600 mt-2" role="alert">{sendError}</p>
      {/if}
      {#if sendStatus}
        <p class="text-sm text-green-700 mt-2">{sendStatus}</p>
      {/if}
    </section>

    <section class="bg-white border border-gray-200 rounded-xl p-5">
      <h2 class="text-sm font-semibold mb-3">
        Pending requests {#if requests.length > 0}<span class="text-gray-400 font-normal">{requests.length}</span
          >{/if}
      </h2>
      {#if requestsError}
        <p class="text-sm text-red-600" role="alert">{requestsError}</p>
      {:else if requestsLoading}
        <p class="text-sm text-gray-500">Loading…</p>
      {:else if requests.length === 0}
        <p class="text-sm text-gray-500">No pending requests.</p>
      {:else}
        <div class="space-y-3">
          {#each requests as request (request.id)}
            <div class="flex items-center gap-3 flex-wrap">
              {#if request.from_avatar_url}
                <img src={request.from_avatar_url} alt="" class="w-9 h-9 rounded-full" />
              {:else}
                <div class="w-9 h-9 rounded-full bg-gray-300"></div>
              {/if}
              <span class="flex-1 text-sm font-medium min-w-[100px]">{request.from_username}</span>
              <div class="flex gap-2">
                <button
                  on:click={() => respond(request.id, true)}
                  class="px-3 py-1.5 bg-primary text-white rounded-lg text-xs font-semibold"
                >
                  Accept
                </button>
                <button
                  on:click={() => respond(request.id, false)}
                  class="px-3 py-1.5 border border-gray-300 rounded-lg text-xs font-semibold text-gray-600"
                >
                  Decline
                </button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    {#if missingCount !== null && missingCount > 0}
      <section class="bg-indigo-50 rounded-xl p-5 flex items-center gap-4 flex-wrap">
        <div class="flex-1 min-w-[200px]">
          <p class="text-sm font-semibold m-0">
            {missingCount} member{missingCount === 1 ? '' : 's'} of your server aren't on Friends Calendar yet
          </p>
          {#if inviteError}
            <p class="text-sm text-red-600 mt-1" role="alert">{inviteError}</p>
          {:else if inviteSent}
            <p class="text-sm text-primary mt-1">Invite posted!</p>
          {/if}
        </div>
        <button
          on:click={handlePostInvite}
          disabled={inviteSent}
          class="px-4 py-2 bg-primary text-white rounded-lg text-sm font-semibold disabled:opacity-50"
        >
          Post invite
        </button>
      </section>
    {/if}
  </div>
</Frame>

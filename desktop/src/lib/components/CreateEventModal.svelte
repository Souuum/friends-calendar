<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { FriendInfo, Visibility } from '$lib/types';

  const dispatch = createEventDispatcher();

  let title = '';
  let description = '';
  let startTime = '';
  let endTime = '';
  let location = '';
  let visibility: Visibility = 'friends';
  let loading = false;
  let error = '';
  let price = '';
  let link = '';

  // Invite picker: participant_ids has always been accepted by the
  // backend (services::calendar::create_event), but nothing here ever
  // sent it - meaning nobody but the creator was ever actually invited,
  // regardless of `visibility`. See mockup-friends-directory skill.
  let friends: FriendInfo[] = [];
  let friendsError = '';
  let selectedFriendIds = new Set<string>();

  onMount(async () => {
    try {
      friends = await api.getFriends();
    } catch (err) {
      friendsError = err instanceof Error ? err.message : 'Failed to load friends';
    }
  });

  function toggleFriend(userId: string) {
    const next = new Set(selectedFriendIds);
    if (next.has(userId)) {
      next.delete(userId);
    } else {
      next.add(userId);
    }
    selectedFriendIds = next;
  }

  async function handleSubmit() {
    if (!title || !startTime || !endTime) {
      error = 'Please fill in all required fields';
      return;
    }

    const payload = {
      title,
      description: description || undefined,
      start_time: new Date(startTime).toISOString(),
      end_time: new Date(endTime).toISOString(),
      location: location || undefined,
      visibility,
      price: price || undefined,
      link: link || undefined,
      participant_ids: selectedFriendIds.size > 0 ? Array.from(selectedFriendIds) : undefined
    };
    console.log('📤 Sending payload:', payload);

    try {
      loading = true;
      error = '';
      await api.createEvent(payload);
      dispatch('created');
    } catch (err) {
      console.error('❌ Error:', err);
      error = err instanceof Error ? err.message : 'Failed to create event';
    } finally {
      loading = false;
    }
  }

  function handleClose() {
    dispatch('close');
  }
</script>

<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50 anim-scrim">
  <div class="bg-white rounded-2xl shadow-2xl max-w-2xl w-full max-h-[90vh] overflow-y-auto anim-pop">
    <div class="p-6">
      <div class="flex justify-between items-center mb-6">
        <h2 class="text-2xl font-bold text-gray-900">Create New Event</h2>
        <button on:click={handleClose} class="text-gray-400 hover:text-gray-600 text-2xl">
          ×
        </button>
      </div>

      {#if error}
        <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4">
          {error}
        </div>
      {/if}

      <form on:submit|preventDefault={handleSubmit} class="space-y-4">
        <div>
          <label for="title" class="block text-sm font-medium text-gray-700 mb-1">
            Event Title *
          </label>
          <input
            id="title"
            type="text"
            bind:value={title}
            required
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            placeholder="Team Meeting"
          />
        </div>

        <div>
          <label for="description" class="block text-sm font-medium text-gray-700 mb-1">
            Description
          </label>
          <textarea
            id="description"
            bind:value={description}
            rows="3"
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple
            focus:border-transparent"
            placeholder="What's this event about?"
          >
          </textarea>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label for="startTime" class="block text-sm font-medium text-gray-700 mb-1">
              Start Time *
            </label>
            <input
              id="startTime"
              type="datetime-local"
              bind:value={startTime}
              required
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            />
          </div>

          <div>
            <label for="endTime" class="block text-sm font-medium text-gray-700 mb-1">
              End Time *
            </label>
            <input
              id="endTime"
              type="datetime-local"
              bind:value={endTime}
              required
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            />
          </div>
        </div>

        <div>
          <label for="location" class="block text-sm font-medium text-gray-700 mb-1">
            Location
          </label>
          <input
            id="location"
            type="text"
            bind:value={location}
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            placeholder="Conference Room A"
          />
        </div>
        <div>
          <label for="price" class="block text-sm font-medium text-gray-700 mb-1"> Prix </label>
          <input
            id="price"
            type="text"
            bind:value={price}
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            placeholder="20€ ou Gratuit"
          />
        </div>

        <div>
          <label for="link" class="block text-sm font-medium text-gray-700 mb-1"> Lien </label>
          <input
            id="link"
            type="url"
            bind:value={link}
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            placeholder="https://example.com"
          />
        </div>

        <div>
          <label for="visibility" class="block text-sm font-medium text-gray-700 mb-1">
            Visibility
          </label>
          <select
            id="visibility"
            bind:value={visibility}
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
          >
            <option value="private">Private (only you)</option>
            <option value="friends">Friends</option>
            <option value="public">Public</option>
          </select>
        </div>

        <div>
          <span class="block text-sm font-medium text-gray-700 mb-1">Invite</span>
          {#if friendsError}
            <p class="text-sm text-red-600" role="alert">{friendsError}</p>
          {:else if friends.length === 0}
            <p class="text-sm text-gray-500">No friends synced yet.</p>
          {:else}
            <div class="flex flex-wrap gap-2">
              {#each friends as friend (friend.user_id)}
                {@const selected = selectedFriendIds.has(friend.user_id)}
                <button
                  type="button"
                  on:click={() => toggleFriend(friend.user_id)}
                  aria-pressed={selected}
                  class="inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-sm font-medium border transition {selected
                    ? 'bg-primary text-white border-primary'
                    : 'bg-gray-100 text-gray-700 border-transparent hover:bg-gray-200'}"
                >
                  {#if friend.avatar_url}
                    <img src={friend.avatar_url} alt="" class="w-5 h-5 rounded-full" />
                  {/if}
                  {friend.username}
                </button>
              {/each}
            </div>
          {/if}
        </div>

        <div class="flex gap-3 pt-4">
          <button
            type="button"
            on:click={handleClose}
            class="flex-1 px-4 py-2 border border-gray-300 rounded-lg text-gray-700 hover:bg-gray-50 font-medium transition"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={loading}
            class="flex-1 px-4 py-2 border-2 border-primary text-primary rounded-lg hover:bg-primary hover:text-white font-medium transition disabled:opacity-50"
          >
            {loading ? 'Creating...' : 'Create Event'}
          </button>
        </div>
      </form>
    </div>
  </div>
</div>

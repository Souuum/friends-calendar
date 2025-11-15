<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { api } from '$lib/api';

  const dispatch = createEventDispatcher();

  let title = '';
  let description = '';
  let startTime = '';
  let endTime = '';
  let location = '';
  let visibility: 'private' | 'friends' | 'public' = 'friends';
  let loading = false;
  let error = '';

  async function handleSubmit() {
    if (!title || !startTime || !endTime) {
      error = 'Please fill in all required fields';
      return;
    }

    try {
      loading = true;
      error = '';
      await api.createEvent({
        title,
        description: description || undefined,
        start_time: new Date(startTime).toISOString(),
        end_time: new Date(endTime).toISOString(),
        location: location || undefined,
        visibility,
      });
      dispatch('created');
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to create event';
    } finally {
      loading = false;
    }
  }

  function handleClose() {
    dispatch('close');
  }
</script>

<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50">
  <div class="bg-white rounded-2xl shadow-2xl max-w-2xl w-full max-h-[90vh] overflow-y-auto">
    <div class="p-6">
      <div class="flex justify-between items-center mb-6">
        <h2 class="text-2xl font-bold text-gray-900">Create New Event</h2>
        <button
          on:click={handleClose}
          class="text-gray-400 hover:text-gray-600 text-2xl"
        >
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
          <textarea>
            id="description"
            bind:value={description}
            rows="3"
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
            placeholder="What's this event about?"
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
            class="flex-1 px-4 py-2 bg-discord-blurple text-white rounded-lg hover:bg-blue-600 font-medium transition disabled:opacity-50"
          >
            {loading ? 'Creating...' : 'Create Event'}
          </button>
        </div>
      </form>
    </div>
  </div>
</div>
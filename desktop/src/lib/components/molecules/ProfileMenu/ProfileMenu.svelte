<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher();

  function handleSettings() {
    dispatch('settings');
  }

  function handleLogout() {
    dispatch('logout');
  }

</script>

<div>
  <button class="w-full text-left px-3 py-2 hover:bg-gray-100 rounded" on:click={handleSettings}>
    Settings
  </button>
  <button class="w-full text-left px-3 py-2 hover:bg-gray-100 rounded" on:click={handleLogout}>
    Logout
  </button>
</div>

<script lang="ts">
  export let onSettings = () => {};
  export let onLogout = () => {};
</script>

<div>
  <button class="w-full text-left px-3 py-2 hover:bg-gray-100 rounded" on:click={onSettings}>
    Settings
  </button>
  <button class="w-full text-left px-3 py-2 hover:bg-gray-100 rounded" on:click={onLogout}>
    Logout
  </button>
</div>

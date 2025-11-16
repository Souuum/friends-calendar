<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { user } from '$lib/stores';

  import Header from '$lib/components/organisms/Header.svelte';

  let avatar_url = `https://cdn.discordapp.com/avatars/${$user?.discord_id}/${$user?.avatar}.png`;

  function handleLogout() {
    api.clearToken();
    window.location.reload();
  }
</script>

<div>
  {#if user}
    <Header {avatar_url} {user} on:logout={handleLogout} />
  {/if}
</div>

<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { user } from '$lib/stores';

  import Header from '$lib/components/organisms/Header.svelte';
  import ViewButton from '$lib/components/templates/ViewButton.svelte'

  let avatarUrl = `https://cdn.discordapp.com/avatars/${$user?.discord_id}/${$user?.avatar}.png`;

  let currentView = 'calendar';

  const navItems = [
      { label: 'Calendars', icon: '📅', view: 'calendar' },
      { label: 'Announcement', icon: '🔔', view: 'announcements' }
  ];

  function handleLogout() {
    api.clearToken();
    window.location.reload();
  }

  function setView(view: string) {
    currentView = view;
  }
</script>

<div>
  {#if user}
    <Header {avatarUrl} {user} on:logout={handleLogout} />
  {/if}
  <div class="flex">
    <aside class="w-48 bg-white flex flex-col p-3 gap-2 h-full">
      {#each navItems as item}
        {@const current = currentView === item.view}
        <ViewButton on:click={() => setView(item.view)} {item} {current} />
      {/each}
    </aside>
    <main class="w-14/16 fit-content">
      <slot />
    </main>
  </div>
</div>

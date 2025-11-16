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

  let currentView = 'calendar';

  function setView(view: string) {
    currentView = view;
  }

  const navItems = [
    { label: 'Calendars', icon: '📅', view: 'calendar' },
    { label: 'Announcement', icon: '🔔', view: 'announcements' },
  ];
</script>

<div>
  {#if user}
    <Header {avatar_url} {user} on:logout={handleLogout} />
  {/if}
          <div class="flex">
    <aside class="w-48 bg-white flex flex-col p-3 gap-2 h-full">
      {#each navItems as item}
        <button
          on:click={() => setView(item.view)}
          class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-primary-hover text-left
                 {currentView === item.view ? 'bg-primary-hover font-semibold  text-primary' : ''}"
        >
          <span>{item.icon}</span>
          <span>{item.label}</span>
        </button>
      {/each}
    </aside>
    <main class="w-14/16 fit-content">
        <slot />
    </main>
  </div>
</div>

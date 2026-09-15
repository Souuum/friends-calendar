<script lang="ts">
  import { api } from '$lib/api';
  import { user } from '$lib/stores';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';

  import Header from '$lib/components/organisms/Header.svelte';
  import ViewButton from '$lib/components/templates/ViewButton.svelte';

  let avatarUrl = `https://cdn.discordapp.com/avatars/${$user?.discord_id}/${$user?.avatar}.png`;

  // `view` doubles as the route to navigate to. Previously this was local
  // component state (`currentView`) that nothing else could see or change,
  // so clicking a sidebar item did nothing outside Frame itself - routing
  // through $app/navigation instead makes the sidebar actually switch pages
  // (and highlight correctly on direct navigation/refresh, via $page).
  const navItems = [
    { label: 'Calendars', icon: '📅', view: '/' },
    { label: 'Friends', icon: '👥', view: '/friends' },
    { label: 'Announcement', icon: '🔔', view: '/announcements' }
  ];

  function handleLogout() {
    api.clearToken();
    window.location.reload();
  }
</script>

<div>
  {#if user}
    <Header {avatarUrl} {user} on:logout={handleLogout} />
  {/if}
  <div class="flex">
    <aside class="w-48 bg-white flex flex-col p-3 gap-2 h-full">
      {#each navItems as item}
        {@const current = $page.url.pathname === item.view}
        <ViewButton on:click={() => goto(item.view)} {item} {current} />
      {/each}
    </aside>
    <main class="w-14/16 fit-content">
      <slot />
    </main>
  </div>
</div>

<script lang="ts">
  import { api } from '$lib/api';
  import { user, unreadNotificationCount } from '$lib/stores';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';

  import Header from '$lib/components/organisms/Header.svelte';
  import ViewButton from '$lib/components/templates/ViewButton.svelte';
  import BottomTabBar from '$lib/components/templates/BottomTabBar.svelte';
  import Icon, { type IconName } from '$lib/components/atoms/Icon.svelte';

  // `$:`, not `let`: this used to be computed once at init, when $user is
  // still null because the root route only populates it in onMount. It
  // resolved to ".../avatars/undefined/undefined.png" and never recovered,
  // so the header avatar was broken for everyone - the username next to it
  // looked fine because that reads the store reactively.
  //
  // The `avatar` field is also genuinely null for accounts that never set
  // one, which needs Discord's default-avatar endpoint rather than a URL
  // ending in "null.png". Index is (id >> 22) % 6 for the current username
  // system, per Discord's CDN docs; BigInt because ids exceed 2^53.
  $: avatarUrl = $user?.avatar
    ? `https://cdn.discordapp.com/avatars/${$user.discord_id}/${$user.avatar}.png`
    : `https://cdn.discordapp.com/embed/avatars/${defaultAvatarIndex($user?.discord_id)}.png`;

  function defaultAvatarIndex(discordId: string | undefined): number {
    if (!discordId) return 0;
    try {
      return Number((BigInt(discordId) >> 22n) % 6n);
    } catch {
      // A non-numeric id (test fixtures, a malformed record) shouldn't throw
      // and take the whole shell down over an avatar.
      return 0;
    }
  }

  // `view` doubles as the route to navigate to. Previously this was local
  // component state (`currentView`) that nothing else could see or change,
  // so clicking a sidebar item did nothing outside Frame itself - routing
  // through $app/navigation instead makes the sidebar actually switch pages
  // (and highlight correctly on direct navigation/refresh, via $page).
  // Reactive (not `const`) so the Notifications badge updates once
  // Header.svelte's onMount populates $unreadNotificationCount.
  type NavItem = { label: string; icon: IconName; view: string; badge?: number };

  let navItems: NavItem[];
  $: navItems = [
    { label: 'Calendars', icon: 'calendar', view: '/' },
    { label: 'Friends', icon: 'friends', view: '/friends' },
    { label: 'Announcement', icon: 'announcements', view: '/announcements' },
    {
      label: 'Notifications',
      icon: 'notifications',
      view: '/notifications',
      badge: $unreadNotificationCount
    },
    { label: 'Discord server', icon: 'bot', view: '/server' }
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
    <!-- testid: EventPeekPanel is also an <aside>, so the tag alone can't
         identify the sidebar for the layout tests. -->
    <aside data-testid="sidebar" class="hidden md:flex w-48 bg-white flex-col p-3 gap-2 h-full">
      {#each navItems as item}
        {@const current = $page.url.pathname === item.view}
        <ViewButton on:click={() => goto(item.view)} {item} {current} />
      {/each}
    </aside>
    <main class="w-full md:w-14/16 fit-content pb-20 md:pb-0">
      <slot />
    </main>
  </div>
  <BottomTabBar />
</div>

<script lang="ts">
  import { api } from '$lib/api';
  import { user, unreadNotificationCount } from '$lib/stores';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';

  import Header from '$lib/components/organisms/Header.svelte';
  import ViewButton from '$lib/components/templates/ViewButton.svelte';
  import BottomTabBar from '$lib/components/templates/BottomTabBar.svelte';

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
  type NavItem = { label: string; view: string; badge?: number };

  // Order and labels are the mockup's own `navDefs`. Three routes it lists
  // were missing here and reachable only from inside another page: Add
  // friends, Discord server and Settings.
  //
  // The mockup's "New event" row is deliberately not reproduced: in this app
  // that is a modal owned by the calendar, not a route, so a nav item could
  // only navigate to the calendar without opening anything - a control that
  // looks like it does something and doesn't, which is the exact failure
  // this codebase keeps having to undo.
  let navItems: NavItem[];
  $: navItems = [
    { label: 'Calendar', view: '/' },
    { label: 'Friends', view: '/friends' },
    { label: 'Add friends', view: '/friends/add' },
    { label: 'Announcements', view: '/announcements' },
    { label: 'Notifications', view: '/notifications', badge: $unreadNotificationCount },
    { label: 'Discord server', view: '/server' },
    { label: 'Settings', view: '/settings' }
  ];

  // The mockup's header names the screen you're on. `/friends/[id]` shows
  // the friend's name there; the page itself knows that and nothing here
  // does, so this falls back to the section.
  const TITLES: Record<string, string> = {
    '/': 'Calendar',
    '/friends': 'Friends',
    '/friends/add': 'Add friends',
    '/announcements': 'Announcements',
    '/notifications': 'Notifications',
    '/server': 'Discord server',
    '/servers': 'Servers',
    '/settings': 'Settings'
  };

  $: screenTitle =
    TITLES[$page.url.pathname] ??
    (($page.url.pathname.startsWith('/friends/') && 'Friends') ||
      ($page.url.pathname.startsWith('/announcements/') && 'Announcements') ||
      '');

  function handleLogout() {
    api.clearToken();
    window.location.reload();
  }
</script>

<!-- The mockup's shell: a full-height rail beside a column that owns its
     own header, rather than a header spanning both. -->
<!-- md:h-screen + the scroll container below: the mockup's shell is exactly
     viewport height with only the content scrolling, which is what keeps the
     rail's footer pinned to the bottom of the screen rather than to the
     bottom of whatever the page happens to be. Below md: the page scrolls
     normally and the rail isn't rendered at all. -->
<div class="flex flex-col md:h-screen md:overflow-hidden">
  <div class="flex min-h-0 flex-1">
    <!-- testid: EventPeekPanel is also an <aside>, so the tag alone can't
         identify the sidebar for the layout tests. 216px and the 12/14px
         padding are the mockup's. -->
    <aside
      data-testid="sidebar"
      class="hidden md:flex w-[216px] shrink-0 flex-col gap-0.5 border-r border-line bg-surface px-3 py-3.5"
    >
      <div class="px-2.5 pb-2.5 pt-1.5 font-mono text-[10px] uppercase tracking-[0.1em] text-muted">
        Navigate
      </div>
      {#each navItems as item}
        {@const current = $page.url.pathname === item.view}
        <ViewButton on:click={() => goto(item.view)} {item} {current} />
      {/each}

      <!-- The mockup puts the signed-in user at the foot of the rail, not in
           the header. Real avatar rather than the mockup's initials: that's
           data we have, and initials would be a downgrade. -->
      {#if $user}
        <div class="mt-auto flex items-center gap-2.5 border-t border-line px-2.5 pb-1 pt-3">
          <img src={avatarUrl} alt="" class="h-[30px] w-[30px] shrink-0 rounded-full" />
          <div class="min-w-0">
            <div class="truncate text-[13px] font-semibold">{$user.username}</div>
            <div class="font-mono text-[10px] text-muted">connected</div>
          </div>
        </div>
      {/if}
    </aside>

    <div class="flex min-w-0 flex-1 flex-col">
      {#if user}
        <Header {avatarUrl} {user} title={screenTitle} on:logout={handleLogout} />
      {/if}
      <!-- min-w-0: a flex child defaults to `min-width: auto`, so it refuses to
         shrink below its content's minimum. With the 192px sidebar beside it,
         `md:w-14/16` (87.5%) wants 864px inside a 768px viewport, and without
         this it simply overflows instead of shrinking. Month view hid the
         problem because its content is narrow enough to shrink on its own;
         the week grid is not. -->
      <!-- One container for every screen, at the mockup's own 1080px /
           26px-24px-60px. Pages used to each carry their own max-width and
           padding, which is why no two agreed on either. -->
      <main
        class="min-w-0 flex-1 overflow-x-hidden px-4 pb-24 pt-5 md:overflow-y-auto md:px-6 md:pb-[60px] md:pt-[26px]"
      >
        <div class="mx-auto w-full max-w-[1080px]">
          <slot />
        </div>
      </main>
    </div>
  </div>
  <BottomTabBar />
</div>

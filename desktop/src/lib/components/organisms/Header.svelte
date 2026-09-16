<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '$lib/components/atoms/Button.svelte';
  import ProfileMenuTrigger from '$lib/components/molecules/ProfileMenu/ProfileMenuTrigger.svelte';
  import ProfileMenu from '$lib/components/molecules/ProfileMenu/ProfileMenu.svelte';
  import { clickOutside } from '$lib/actions/clickOutside';
  import { api } from '$lib/api';
  import { goto } from '$app/navigation';
  import { unreadNotificationCount } from '$lib/stores';
  import { resolvedTheme, toggleTheme } from '$lib/theme';

  export let user;
  export let avatarUrl: string;

  let show = false;
  let showCreateModal = false;

  function toggleMenu() {
    show = !show;
  }

  function goToSettings() {
    show = false;
    goto('/settings');
  }

  function goToNotifications() {
    goto('/notifications');
  }

  function handleLogout() {
    api.clearToken();
    window.location.reload();
  }

  onMount(async () => {
    if (!user) return;
    try {
      unreadNotificationCount.set(await api.getUnreadNotificationCount());
    } catch {
      // Non-critical - the header shouldn't break if this one call fails.
    }
  });

  $: username = $user?.username;
</script>

<header class="bg-white">
  <div class="mx-auto px-4 pt-2 sm:px-6 lg:px-8 flex items-center justify-end gap-3">
    {#if user}
      <!-- Icon shows what you'd switch TO, which is the prevailing
           convention (moon while light, sun while dark). The aria-label
           says it outright so the icon doesn't have to carry that alone.
           Reads $resolvedTheme, not $theme: on 'system' the stored value
           isn't what's on screen. -->
      <button
        on:click={toggleTheme}
        data-testid="theme-toggle"
        aria-label={$resolvedTheme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
        title={$resolvedTheme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
        class="w-9 h-9 flex items-center justify-center border border-gray-200 rounded-lg hover:bg-gray-50"
      >
        {$resolvedTheme === 'dark' ? '☀️' : '🌙'}
      </button>

      <button
        on:click={goToNotifications}
        aria-label="Notifications"
        class="relative w-9 h-9 flex items-center justify-center border border-gray-200 rounded-lg hover:bg-gray-50"
      >
        🔔
        {#if $unreadNotificationCount > 0}
          <span
            data-testid="unread-dot"
            class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-red-500"
          ></span>
        {/if}
      </button>
      <ProfileMenuTrigger {username} {show} avatar={avatarUrl} on:click={toggleMenu} />
    {/if}

    {#if show}
      {console.log('showing profile menu')}
      <div
        class="absolute w-48 bg-white shadow-lg rounded-lg p-2 top-16 z-50"
        use:clickOutside={() => (show = false)}
      >
        <ProfileMenu on:settings={goToSettings} on:logout={handleLogout} />
      </div>
    {/if}
  </div>
</header>

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
  import Icon from '$lib/components/atoms/Icon.svelte';

  export let user;
  export let avatarUrl: string;
  /** The screen you're on, as the mockup's header shows it. */
  export let title = '';

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

<!-- Mockup header: the screen name on the left, actions on the right, on a
     surface band with a hairline under it. 34px controls at radius 9 are its
     values. -->
<header class="border-b border-line bg-surface px-4 py-3 md:px-6">
  <div class="flex items-center gap-3.5">
    <h1 class="m-0 text-[15px] font-semibold">{title}</h1>
    <div class="ml-auto flex items-center gap-2.5">
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
          class="flex h-[34px] w-[34px] items-center justify-center rounded-[9px] border border-line bg-surface hover:bg-subtle"
        >
          <Icon name={$resolvedTheme === 'dark' ? 'light-mode' : 'dark-mode'} />
        </button>

        <button
          on:click={goToNotifications}
          aria-label="Notifications"
          class="relative flex h-[34px] w-[34px] items-center justify-center rounded-[9px] border border-line bg-surface hover:bg-subtle"
        >
          <Icon name="notifications" />
          {#if $unreadNotificationCount > 0}
            <span
              data-testid="unread-dot"
              class="absolute right-1.5 top-1.5 h-[7px] w-[7px] rounded-full bg-secondary"
            ></span>
          {/if}
        </button>
        <ProfileMenuTrigger {username} {show} avatar={avatarUrl} on:click={toggleMenu} />
      {/if}

      {#if show}
        {console.log('showing profile menu')}
        <div
          class="absolute w-48 bg-surface shadow-lg rounded-lg p-2 top-16 z-50"
          use:clickOutside={() => (show = false)}
        >
          <ProfileMenu on:settings={goToSettings} on:logout={handleLogout} />
        </div>
      {/if}
    </div>
  </div>
</header>

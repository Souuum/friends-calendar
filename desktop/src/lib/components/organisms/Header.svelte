<script lang="ts">
  import Button from '$lib/components/atoms/Button.svelte';
  import ProfileMenuTrigger from '$lib/components/molecules/ProfileMenu/ProfileMenuTrigger.svelte';
  import ProfileMenu from '$lib/components/molecules/ProfileMenu/ProfileMenu.svelte';
  import { clickOutside } from '$lib/actions/clickOutside';
  import { api } from '$lib/api';

  export let user;
  export let avatarUrl: string;

  let show = false;
  let showCreateModal = false;

  function toggleMenu() {
    show = !show;
  }

  function goToSettings() {
    console.log('settings clicked');
  }

  function handleLogout() {
    api.clearToken();
    window.location.reload();
  }

  $: username = $user?.username;
</script>

<header class="bg-white">
  <div class="mx-auto px-4 pt-2 sm:px-6 lg:px-8 flex justify-end">
    {#if user}
      <ProfileMenuTrigger
        {username}
        {show}
        avatar={avatarUrl}
        on:click={toggleMenu}
      />
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

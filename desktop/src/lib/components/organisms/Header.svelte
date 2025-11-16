<script lang="ts">
  import Button from "$lib/components/atoms/Button.svelte";
  import ProfileMenuTrigger from "$lib/components/molecules/ProfileMenu/ProfileMenuTrigger.svelte";
  import ProfileMenu from "$lib/components/molecules/ProfileMenu/ProfileMenu.svelte";
  import { clickOutside } from "$lib/actions/clickOutside";

  export let user;
  export let avatar_url;

  let showMenu = false;
  let showCreateModal = false;

  function toggleMenu() {
    console.log("toggling menu");
    showMenu = !showMenu;
  }

  function goToSettings() {
    console.log("settings clicked");
  }

  function logout() {
    console.log("logout clicked");
  }
</script>

<header class="bg-white shadow">
  <div class="mx-auto px-4 py-4 sm:px-6 lg:px-8 flex justify-end">

      {#if user}
        <ProfileMenuTrigger
          username={$user.username}
          avatar={avatar_url}
          onClick={toggleMenu}
        />
      {/if}

      {#if showMenu}
      {console.log("showing profile menu")}
      <div class="absolute w-48 bg-white shadow-lg rounded-lg p-2 mt-16"
        use:clickOutside={() => (showMenu = false)}>
          <ProfileMenu
            onSettings={goToSettings}
            onLogout={logout}
          />
        </div>
      {/if}
  </div>
</header>

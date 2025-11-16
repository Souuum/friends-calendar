<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { isAuthenticated, isLoading } from '$lib/stores';
  import { api } from '$lib/api';
  import { user } from '$lib/stores';
  import LoginScreen from '$lib/components/LoginScreen.svelte';
  import CalendarView from '$lib/components/CalendarView.svelte';
  import Frame from '$lib/components/templates/Frame.svelte';

  onMount(async () => {
    // Check if token is in URL (from Discord callback)
    const urlParams = new URLSearchParams(window.location.search);
    const tokenFromUrl = urlParams.get('token');

    if (tokenFromUrl) {
      console.log('Token found in URL, logging in...');
      try {
        api.setToken(tokenFromUrl);
        const userData = await api.getCurrentUser();
        user.set(userData);
        isAuthenticated.set(true);

        // Clean URL (remove token from URL bar)
        window.history.replaceState({}, document.title, window.location.pathname);
      } catch (err) {
        console.error('Token from URL invalid:', err);
        api.clearToken();
        isAuthenticated.set(false);
      }
    } else {
      // Check for existing token in localStorage
      const token = api.getToken();

      if (token) {
        try {
          const userData = await api.getCurrentUser();
          user.set(userData);
          isAuthenticated.set(true);
        } catch (err) {
          console.error('Stored token invalid:', err);
          api.clearToken();
          isAuthenticated.set(false);
        }
      } else {
        isAuthenticated.set(false);
      }
    }

    isLoading.set(false);
  });
</script>

<svelte:head>
  <title>Friends Calendar</title>
</svelte:head>

<main>
  {#if $isLoading}
    <div class="flex items-center justify-center min-h-screen bg-gray-50">
      <div class="text-center">
        <div
          class="animate-spin rounded-full h-16 w-16 border-4 border-discord-blurple border-t-transparent mx-auto mb-4"
        ></div>
        <p class="text-gray-600">Loading...</p>
      </div>
    </div>
  {:else if $isAuthenticated}
  <Frame>
    <CalendarView />
  </Frame>
  {:else}
    <LoginScreen />
  {/if}
</main>

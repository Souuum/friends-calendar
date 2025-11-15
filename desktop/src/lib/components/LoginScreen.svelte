<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { user, isAuthenticated, isLoading } from '$lib/stores';

  let error = '';
  let tokenInput = '';
  let showTokenInput = false;

  async function handleLogin() {
    try {
      // Open Discord OAuth in browser
      const authUrl = 'http://localhost:8080/api/auth/discord';
      window.open(authUrl, '_blank');
      
      // Show token input
      showTokenInput = true;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Login failed';
    }
  }

  async function handleTokenSubmit() {
    if (!tokenInput.trim()) {
      error = 'Please enter a token';
      return;
    }

    try {
      isLoading.set(true);
      api.setToken(tokenInput.trim());
      await loadUser();
      console.log('Login successful, user :', $user);
      showTokenInput = false;
      tokenInput = '';
    } catch (err) {
      error = err instanceof Error ? err.message : 'Invalid token';
      api.clearToken();
    } finally {
      isLoading.set(false);
    }
  }

  async function loadUser() {
    const userData = await api.getCurrentUser();
    user.set(userData);
    isAuthenticated.set(true);
  }

  onMount(async () => {
    const token = api.getToken();
    if (token) {
      try {
        await loadUser();
      } catch (err) {
        console.error('Token invalid, need to login again');
        api.clearToken();
      }
    }
    isLoading.set(false);
  });
</script>

<div class="flex items-center justify-center min-h-screen bg-gradient-to-br from-discord-blurple to-discord-fuchsia">
  <div class="bg-white rounded-2xl shadow-2xl p-8 max-w-md w-full mx-4">
    <div class="text-center mb-8">
      <h1 class="text-4xl font-bold text-discord-blurple mb-2">📅 Friends Calendar</h1>
      <p class="text-gray-600">Sync your schedule with friends</p>
    </div>

    {#if error}
      <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4">
        {error}
      </div>
    {/if}

    {#if !showTokenInput}
      <button
        on:click={handleLogin}
        class="w-full bg-discord-blurple hover:bg-blue-600 text-white font-bold py-3 px-4 rounded-lg transition duration-200 flex items-center justify-center gap-2"
      >
        <svg class="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
          <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515a.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0a12.64 12.64 0 0 0-.617-1.25a.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057a19.9 19.9 0 0 0 5.993 3.03a.078.078 0 0 0 .084-.028a14.09 14.09 0 0 0 1.226-1.994a.076.076 0 0 0-.041-.106a13.107 13.107 0 0 1-1.872-.892a.077.077 0 0 1-.008-.128a10.2 10.2 0 0 0 .372-.292a.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127a12.299 12.299 0 0 1-1.873.892a.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028a19.839 19.839 0 0 0 6.002-3.03a.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.956-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.955-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.946 2.418-2.157 2.418z"/>
        </svg>
        Login with Discord
      </button>
    {:else}
      <div class="space-y-4">
        <p class="text-sm text-gray-600">
          After authorizing in Discord, copy the token from the URL and paste it below:
        </p>
        <input
          type="text"
          bind:value={tokenInput}
          placeholder="Paste your JWT token here"
          class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-discord-blurple focus:border-transparent"
        />
        <div class="flex gap-2">
          <button
            on:click={() => { showTokenInput = false; tokenInput = ''; }}
            class="flex-1 px-4 py-2 border border-gray-300 rounded-lg text-gray-700 hover:bg-gray-50"
          >
            Cancel
          </button>
          <button
            on:click={handleTokenSubmit}
            class="flex-1 px-4 py-2 bg-discord-blurple text-white rounded-lg hover:bg-blue-600"
          >
            Submit
          </button>
        </div>
      </div>
    {/if}

    <div class="mt-6 text-center text-sm text-gray-500">
      <p>Secure login powered by Discord OAuth2</p>
    </div>
  </div>
</div>
<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { watchSystemTheme } from '$lib/theme';

  // The class itself is already set by the inline script in app.html; this
  // only keeps 'system' following the OS if it changes while the app is
  // open. onMount treats the returned teardown as its cleanup.
  onMount(watchSystemTheme);
</script>

<slot />

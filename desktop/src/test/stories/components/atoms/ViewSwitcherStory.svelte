<script lang="ts">
  import ViewSwitcher from '$lib/components/atoms/ViewSwitcher.svelte';
  import type { ViewType } from '$lib/types';

  export let view: ViewType = 'month';
</script>

<ViewSwitcher bind:view />

<pre>view : {view}</pre>

<script lang="ts">
  import ViewSwitcher from '$lib/components/atoms/ViewSwitcher.svelte';
  import type { ViewType } from '$lib/types';

  /** The view selected when the story opens. */
  export let view: ViewType = 'month';

  let lastDetail: ViewType | null = null;
</script>

<ViewSwitcher bind:view on:view-change={(e) => (lastDetail = e.detail)} />

<p class="mt-3 font-mono text-xs text-muted">bind:view = {view}</p>
<p class="font-mono text-xs text-muted">on:view-change detail = {lastDetail ?? '(none yet)'}</p>

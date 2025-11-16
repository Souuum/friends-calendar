<script lang="ts">
  import type { ViewType } from '$lib/types';
  import IconButton from '$lib/components/atoms/IconButton.svelte';
  import ViewSwitcher from '$lib/components/atoms/ViewSwitcher.svelte';

  export let title: string;
  export let view: ViewType;
  export let onPrev: () => void;
  export let onNext: () => void;
  export let onToday: () => void;
  export let onViewChange: (view: ViewType) => void;
</script>

<div class="flex items-center justify-between p-6 border-b">
  <div class="flex items-center gap-3">
    <IconButton onClick={onPrev} label="Previous" direction="left" />
    <h2 class="text-2xl font-bold">{title}</h2>
    <IconButton onClick={onNext} label="Next" direction="right" />
  </div>

  <div class="flex items-center gap-3">
    <button
      on:click={onToday}
      class="px-4 py-2 text-sm font-medium hover:bg-gray-100 rounded-lg transition-colors"
    >
      Today
    </button>
    <ViewSwitcher {view} onChange={onViewChange} />
  </div>
</div>

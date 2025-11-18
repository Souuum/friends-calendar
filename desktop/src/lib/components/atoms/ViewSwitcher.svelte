<script lang="ts">
  import type { ViewType } from '$lib/types';
  import { createEventDispatcher } from 'svelte';
  
  export let view: ViewType;

  const dispatch = createEventDispatcher();

  function onChange(view: ViewType) {
    dispatch('view-change', view);
  }

  const views: { value: ViewType; label: string }[] = [
    { value: 'day', label: 'Day' },
    { value: 'week', label: 'Week' },
    { value: 'month', label: 'Month' }
  ];
</script>

<div class="flex bg-gray-100 rounded-lg p-1">
  {#each views as viewOption}
    {@const isActive = view === viewOption.value}
    <button
      on:click={() => onChange(viewOption.value)}
      class="px-3 py-1 text-sm rounded transition-colors"
      class:bg-white={isActive}
      class:shadow-sm={isActive}
      class:font-semibold={isActive}
      class:text-primary={isActive}
    >
      {viewOption.label}
    </button>
  {/each}
</div>

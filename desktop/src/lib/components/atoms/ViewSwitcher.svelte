<script lang="ts">
  import type { ViewType } from '$lib/types';
  import { createEventDispatcher } from 'svelte';

  export let view: ViewType;

  const dispatch = createEventDispatcher();

  function onChange(viewType: ViewType) {
    view = viewType;
    dispatch('view-change', viewType);
  }

  const views: { value: ViewType; label: string }[] = [
    { value: 'day', label: 'Day' },
    { value: 'week', label: 'Week' },
    { value: 'month', label: 'Month' }
  ];
</script>

<div class="flex bg-[#f4f4f7] rounded-[9px] p-[3px]">
  {#each views as viewOption}
    {@const isActive = view === viewOption.value}
    <button
      on:click={() => onChange(viewOption.value)}
      class="px-[13px] py-1.5 text-[13px] rounded-[7px] transition-colors cursor-pointer"
      class:bg-white={isActive}
      class:shadow-sm={isActive}
      class:font-semibold={isActive}
      class:text-primary={isActive}
      class:font-medium={!isActive}
      class:text-muted={!isActive}
    >
      {viewOption.label}
    </button>
  {/each}
</div>

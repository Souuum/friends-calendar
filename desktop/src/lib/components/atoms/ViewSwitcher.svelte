<script lang="ts">
  import type { ViewType } from '$lib/types';

  export let view: ViewType;
  export let onChange: (view: ViewType) => void;

  const views: { value: ViewType; label: string }[] = [
    { value: 'day', label: 'Day' },
    { value: 'week', label: 'Week' },
    { value: 'month', label: 'Month' }
  ];
</script>

<div class="flex bg-gray-100 rounded-lg p-1">
  {#each views as viewOption}
    <button
      on:click={() => onChange(viewOption.value)}
      class="px-3 py-1 text-sm rounded transition-colors"
      class:bg-white={view === viewOption.value}
      class:shadow-sm={view === viewOption.value}
      class:font-semibold={view === viewOption.value}>
      {viewOption.label}
    </button>
  {/each}
</div>
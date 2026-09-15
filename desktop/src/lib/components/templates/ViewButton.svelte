<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let item: { label: string; icon: string; view: string; badge?: number };
  export let current: boolean = false;

  const dispatch = createEventDispatcher();
</script>

<button
  on:click
  class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-primary-hover text-left
        {current ? 'bg-primary-hover font-semibold  text-primary' : ''}"
>
  <span>{item.icon}</span>
  <span class="flex-1">{item.label}</span>
  {#if item.badge}
    <span
      class="font-mono text-[10px] px-1.5 py-0.5 rounded-full {current
        ? 'bg-primary text-white'
        : 'bg-gray-200 text-gray-600'}"
    >
      {item.badge}
    </span>
  {/if}
</button>

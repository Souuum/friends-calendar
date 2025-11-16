<script lang="ts">
  export let onClick: () => void;
  export let label: string;
  export let direction: 'left' | 'right' = 'left';
</script>

<button
  on:click={onClick}
  class="p-2 hover:bg-gray-100 rounded-lg transition-colors"
  aria-label={label}>
  {#if direction === 'left'}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
    </svg>
  {:else}
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
    </svg>
  {/if}
</button>
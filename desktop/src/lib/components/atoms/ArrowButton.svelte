<script lang="ts">
  export let label: string;
  export let direction: 'left' | 'right' = 'left';

  $: commands = direction === 'left' ? 'M15 19l-7-7 7-7' : 'M9 5l7 7-7 7';
</script>

<button on:click class="p-2 hover:bg-primary-hover rounded-lg transition-colors" aria-label={label}>
  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={commands} />
  </svg>
</button>

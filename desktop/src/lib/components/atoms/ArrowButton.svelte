<script lang="ts">
  /** Accessible name, used as `aria-label` - the button has no visible text. */
  export let label: string;
  /** Which way the chevron points. */
  export let direction: 'left' | 'right' = 'left';

  $: commands = direction === 'left' ? 'M15 19l-7-7 7-7' : 'M9 5l7 7-7 7';
</script>

<button
  on:click
  class="w-8 h-8 flex items-center justify-center border border-line bg-surface text-muted rounded-lg hover:bg-gray-50 transition-colors"
  aria-label={label}
>
  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={commands} />
  </svg>
</button>

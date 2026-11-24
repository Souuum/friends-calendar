<script lang="ts">
  /** The hour this cell stands for. Not rendered - the cell looks the same for every hour. */
  export let hour: number;
  /** Tints the cell to show that something happens during this hour. */
  export let hasEvents = false;
</script>

<div
  class="h-20 border-b border-l border-gray-200 hover:bg-gray-50 transition-colors relative"
  class:bg-blue-50={hasEvents}
>
  <slot />
</div>

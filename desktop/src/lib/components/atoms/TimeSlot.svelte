<script lang="ts">
  export let hour: number;
  export let hasEvents = false;
</script>

<div
  class="h-20 border-b border-l border-gray-200 hover:bg-gray-50 transition-colors relative"
  class:bg-blue-50={hasEvents}
>
  <slot />
</div>

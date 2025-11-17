<script lang="ts">
  export let weekdays: string[] = [
    'Monday',
    'Tuesday',
    'Wednesday',
    'Thursday',
    'Friday',
    'Saturday',
    'Sunday'
  ];
  export let short = false;
</script>

<div class="grid grid-cols-7 mb-2">
  {#each weekdays as day}
    <div class="text-center py-3">
      <span class="text-sm font-medium text-gray-600">
        {short ? day.slice(0, 3) : day}
      </span>
    </div>
  {/each}
</div>

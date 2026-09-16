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
  /** Force the abbreviated form at every width. */
  export let short = false;

  $: abbreviated = weekdays.map((day) => day.slice(0, 3));
</script>

<div class="grid grid-cols-7 mb-2">
  {#each weekdays as day, i}
    <div class="text-center py-3">
      <span class="text-sm font-medium text-gray-600">
        {#if short}
          {abbreviated[i]}
        {:else}
          <!-- Seven columns leave ~50px each at 402px, which full weekday
               names overflow - they ran into each other rather than
               widening the page, so no overflow assertion caught it. -->
          <span class="md:hidden">{abbreviated[i]}</span>
          <span class="hidden md:inline">{day}</span>
        {/if}
      </span>
    </div>
  {/each}
</div>

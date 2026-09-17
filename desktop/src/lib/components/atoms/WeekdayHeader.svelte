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

<!-- 12px/600 muted with 4px of vertical padding, and 6px under the row -
     the mockup's values. This was `text-sm` (14px) at `py-3`, which made the
     weekday strip heavier than the day numbers under it. -->
<div class="mb-1.5 grid grid-cols-7">
  {#each weekdays as day, i}
    <div class="py-1 text-center">
      <span class="text-[12px] font-semibold text-muted">
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

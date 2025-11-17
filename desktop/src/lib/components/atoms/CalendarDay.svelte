<script lang="ts">
  export let date: Date;
  export let isToday = false;
  export let isCurrentMonth = true;
  export let size: 'small' | 'large' = 'large';

  $: isLarge = size === 'small';
  $: isSmall = !isLarge;
</script>

<span
  class="transition-colors"
  class:text-sm={isSmall}
  class:text-lg={isLarge}
  class:font-medium={isSmall}
  class:mt-1={isLarge}
  class:text-primary={isToday && isLarge}
  class:bg-primary={isToday}
  class:text-white={isToday}
  class:rounded-full={isToday}
  class:w-7={isToday && isSmall}
  class:h-7={isToday && isSmall}
  class:w-8={isToday && isLarge}
  class:h-8={isToday && isLarge}
  class:flex={isToday}
  class:items-center={isToday}
  class:justify-center={isToday}
  class:opacity-60={!isCurrentMonth && isSmall}
>
  {date.getDate()}
</span>

<script lang="ts">
  export let date: Date;
  export let isToday = false;
  export let isCurrentMonth = true;
  export let size: 'small' | 'large' = 'large';
</script>

<span
  class="transition-colors"
  class:text-sm={size === 'small'}
  class:text-lg={size === 'large'}
  class:font-medium={size === 'small'}
  class:mt-1={size === 'large'}
  class:text-primary={isToday && size === 'large'}
  class:bg-primary={isToday}
  class:text-white={isToday}
  class:rounded-full={isToday}
  class:w-7={isToday && size === 'small'}
  class:h-7={isToday && size === 'small'}
  class:w-8={isToday && size === 'large'}
  class:h-8={isToday && size === 'large'}
  class:flex={isToday}
  class:items-center={isToday}
  class:justify-center={isToday}
  class:opacity-60={!isCurrentMonth && size === 'small'}
>
  {date.getDate()}
</span>

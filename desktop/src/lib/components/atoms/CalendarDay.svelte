<script lang="ts">
  export let date: Date;
  export let isToday = false;
  export let isCurrentMonth = true;
  export let size: 'small' | 'large' = 'large';

  $: isLarge = size === 'large';
</script>

<!--
  The mockup's day number: a fixed 25x25 box so numbers line up whether or
  not today's circle is drawn, 13px, 600 in-month and 400 out. The circle is
  the accent fill, never an accent *text* colour on no background - that read
  as a link.
-->
<span
  class="flex shrink-0 items-center justify-center rounded-full
    {isLarge ? 'h-[25px] w-[25px] text-[13px]' : 'h-[22px] w-[22px] text-[12px]'}
    {isToday
    ? 'bg-primary font-bold text-white'
    : isCurrentMonth
      ? 'font-semibold text-ink'
      : 'font-normal text-ink'}"
>
  {date.getDate()}
</span>

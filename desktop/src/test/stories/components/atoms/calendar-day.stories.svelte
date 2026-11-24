<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';

  /**
   * The day-of-month number of a calendar cell, in a fixed round box so the
   * numbers line up whether or not today's circle is drawn.
   *
   * ```svelte
   * <CalendarDay date={day} isToday={isToday(day)} isCurrentMonth={!outside} />
   * ```
   *
   * It only draws the number. Dimming a cell outside the month is up to the
   * caller: `isCurrentMonth={false}` changes the font weight and nothing else.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/CalendarDay',
    component: CalendarDay,
    argTypes: {
      size: { control: 'inline-radio', options: ['small', 'large'] }
    }
  });
</script>

<!-- `size="large"`: the 25px box used by the month grid, for an ordinary day. -->
<Story
  name="Large date"
  args={{
    date: new Date('2024-06-15'),
    isCurrentMonth: true,
    isToday: false,
    size: 'large'
  }}
/>

<!-- `isToday` fills the circle with the accent colour. -->
<Story
  name="Large today"
  args={{
    date: new Date('2024-06-15'),
    isCurrentMonth: true,
    isToday: true,
    size: 'large'
  }}
/>

<!-- `size="small"`: the 22px box used by the week view's day strip on narrow screens. -->
<Story
  name="Small date"
  args={{
    date: new Date('2024-06-15'),
    isCurrentMonth: true,
    isToday: false,
    size: 'small'
  }}
/>

<!-- `size="small"` together with `isToday`. -->
<Story
  name="Small today date"
  args={{
    date: new Date('2024-06-15'),
    isCurrentMonth: true,
    isToday: true,
    size: 'small'
  }}
/>

<!-- `isCurrentMonth={false}`: a day of the neighbouring month, in a lighter font weight. -->
<Story
  name="Outside the month"
  args={{
    date: new Date('2024-05-31'),
    isCurrentMonth: false,
    isToday: false,
    size: 'large'
  }}
/>

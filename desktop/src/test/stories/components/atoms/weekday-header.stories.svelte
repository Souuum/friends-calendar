<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import WeekdayHeader from '$lib/components/atoms/WeekdayHeader.svelte';

  /**
   * The row of seven weekday labels above the month grid.
   *
   * ```svelte
   * <WeekdayHeader />
   * <div class="grid grid-cols-7">
   *   {#each days as day (day.getTime())}
   *     <CalendarDay date={day} />
   *   {/each}
   * </div>
   * ```
   *
   * It lays the labels out in seven equal columns, so it lines up with a
   * seven-column grid of the same width. Below the `md` breakpoint each label
   * is cut to its first three letters; `short` does that at every width.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/WeekdayHeader',
    component: WeekdayHeader
  });
</script>

<!-- No props: the English weekdays from Monday, in full on a wide canvas. -->
<Story name="Default" />

<!-- `short` shows the three-letter form at every width. -->
<Story name="Short" args={{ short: true }} />

<!-- `weekdays` replaces the labels, here with French ones. -->
<Story
  name="Custom labels"
  args={{
    weekdays: ['Lundi', 'Mardi', 'Mercredi', 'Jeudi', 'Vendredi', 'Samedi', 'Dimanche']
  }}
/>

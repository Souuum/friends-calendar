<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import type { ComponentProps } from 'svelte';
  import TimeLabel from '$lib/components/atoms/TimeLabel.svelte';

  /**
   * The hour label in the gutter of the day and week grids, in 12-hour form.
   *
   * ```svelte
   * {#each hours as hour (hour)}
   *   <TimeLabel {hour} compact />
   *   <TimeSlot {hour} />
   * {/each}
   * ```
   *
   * `hour` is 0 to 23. `compact` drops the minutes ("9 AM" rather than
   * "09:00 AM") for narrow gutters. The alignment is a class assembled at
   * runtime from `align`, so it only shows where the stylesheet already
   * contains that class.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/TimeLabel',
    component: TimeLabel,
    argTypes: {
      align: { control: 'inline-radio', options: ['left', 'right'] }
    }
  });
</script>

<!-- A fixed-width box stands in for the grid's gutter, so the alignment has room to show. -->
{#snippet inGutter(args: ComponentProps<typeof TimeLabel>)}
  <div class="w-24 border border-line py-1">
    <TimeLabel {...args} />
  </div>
{/snippet}

<!-- A morning `hour` in the default long form: "09:00 AM". -->
<Story name="Full hour" args={{ hour: 9 }} template={inGutter} />

<!-- An afternoon `hour` is shown on the 12-hour clock: 15 becomes "03:00 PM". -->
<Story name="Afternoon" args={{ hour: 15 }} template={inGutter} />

<!-- `hour={0}` is midnight: "12:00 AM". -->
<Story name="Midnight" args={{ hour: 0 }} template={inGutter} />

<!-- `compact` shortens the label to "9 AM". -->
<Story name="Compact" args={{ hour: 9, compact: true }} template={inGutter} />

<!-- `align="left"` instead of the default `right`. -->
<Story name="Left aligned" args={{ hour: 9, align: 'left' }} template={inGutter} />

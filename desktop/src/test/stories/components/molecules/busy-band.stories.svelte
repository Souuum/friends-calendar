<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import type { ComponentProps } from 'svelte';
  import BusyBand from '$lib/components/molecules/BusyBand.svelte';

  const day = (h: number, m = 0) => {
    const d = new Date();
    d.setHours(h, m, 0, 0);
    return d;
  };

  const block = (start: Date, end: Date, clippedStart = false, clippedEnd = false) => ({
    start,
    end,
    clippedStart,
    clippedEnd
  });

  const { Story } = defineMeta({
    title: 'Components/Molecules/BusyBand',
    component: BusyBand,
    args: { hour: 14, block: block(day(14), day(16)) }
  });
</script>

<!-- Positioned inside an hour row, the way DayView/WeekView draw it. -->
{#snippet hourRow(args: ComponentProps<typeof BusyBand>)}
  <div class="relative h-20 w-64 border-t border-gray-200">
    <BusyBand {...args} />
  </div>
{/snippet}

<Story name="Two hours" template={hourRow} />
<Story
  name="Starts mid-hour"
  args={{ block: block(day(14, 30), day(15, 15)) }}
  template={hourRow}
/>
<Story
  name="Very short (min height)"
  args={{ block: block(day(14), day(14, 5)) }}
  template={hourRow}
/>
<!-- Carried over from last night: labelled "until", not "from". -->
<Story
  name="Continues from the day before"
  args={{ hour: 0, block: block(day(0), day(2), true) }}
  template={hourRow}
/>

<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import type { ComponentProps } from 'svelte';
  import TimedEvent from '$lib/components/molecules/TimedEvent.svelte';
  import { EVENT_HOSTED, EVENT_LONG_TITLE, at } from '../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Molecules/TimedEvent',
    component: TimedEvent,
    args: { event: EVENT_HOSTED, onClick: fn() }
  });
</script>

<!-- Absolutely positioned against its hour row, as in DayView/WeekView. -->
{#snippet hourRow(args: ComponentProps<typeof TimedEvent>)}
  <div class="relative h-20 w-64 border-t border-gray-200">
    <TimedEvent {...args} />
  </div>
{/snippet}

<Story name="Two hours" template={hourRow} />
<Story
  name="Starts at :30"
  args={{ event: { ...EVENT_HOSTED, start_time: at(0, 20, 30), end_time: at(0, 21, 15) } }}
  template={hourRow}
/>
<Story name="Long title" args={{ event: EVENT_LONG_TITLE }} template={hourRow} />

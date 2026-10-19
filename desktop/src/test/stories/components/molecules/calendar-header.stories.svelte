<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import CalendarHeader from '$lib/components/molecules/CalendarHeader.svelte';

  const { Story } = defineMeta({
    title: 'Components/Molecules/CalendarHeader',
    component: CalendarHeader,
    argTypes: { view: { control: 'inline-radio', options: ['month', 'week', 'day', 'list'] } },
    args: {
      title: 'October 2026',
      view: 'month',
      onPrev: fn(),
      onNext: fn(),
      onToday: fn(),
      onNewEvent: fn()
    }
  });
</script>

<Story name="Month" />
<Story name="Week" args={{ view: 'week', title: 'Oct 19 – 25, 2026' }} />
<Story name="Day" args={{ view: 'day', title: 'Monday, October 19, 2026' }} />

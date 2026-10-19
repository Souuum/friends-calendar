<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import TimeSlot from '$lib/components/atoms/TimeSlot.svelte';

  const { Story } = defineMeta({
    title: 'Components/Atoms/TimeSlot',
    component: TimeSlot,
    args: { hour: 9 }
  });
</script>

<Story name="Empty" />

<Story name="Has events" args={{ hasEvents: true }} />

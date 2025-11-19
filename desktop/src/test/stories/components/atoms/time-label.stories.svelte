<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import TimeLabel from '$lib/components/atoms/TimeLabel.svelte';

  const { Story } = defineMeta({
    title: 'Components/Atoms/TimeLabel',
    component: TimeLabel
  });
</script>

<Story name="Full hour" args={{ hour: 24 }} />

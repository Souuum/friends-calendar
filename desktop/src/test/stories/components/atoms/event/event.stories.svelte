<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Event from '$lib/components/atoms/event/Event.svelte';
  import { EVENT_HOSTED, EVENT_LONG_TITLE } from '../../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/Event',
    component: Event,
    args: { event: EVENT_HOSTED }
  });
</script>

<Story name="Default" />

<Story name="Long title" args={{ event: EVENT_LONG_TITLE }} />

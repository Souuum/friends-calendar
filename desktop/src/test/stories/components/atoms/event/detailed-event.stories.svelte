<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import DetailedEvent from '$lib/components/atoms/event/DetailedEvent.svelte';
  import { EVENT_HOSTED, EVENT_LONG_TITLE } from '../../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/DetailedEvent',
    component: DetailedEvent,
    args: { event: EVENT_HOSTED }
  });
</script>

<Story name="Default" />
<Story name="Long title" args={{ event: EVENT_LONG_TITLE }} />

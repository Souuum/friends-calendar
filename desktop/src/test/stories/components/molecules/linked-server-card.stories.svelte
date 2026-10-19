<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import LinkedServerCard from '$lib/components/molecules/LinkedServerCard.svelte';
  import { LINKED_SERVER } from '../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Molecules/LinkedServerCard',
    component: LinkedServerCard,
    args: { server: LINKED_SERVER }
  });
</script>

<Story name="Default" />
<Story
  name="No icon, no member count"
  args={{ server: { id: LINKED_SERVER.id, name: 'Unnamed Server' } }}
/>

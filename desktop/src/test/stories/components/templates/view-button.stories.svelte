<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ViewButton from '$lib/components/templates/ViewButton.svelte';

  const { Story } = defineMeta({
    title: 'Components/Templates/ViewButton',
    component: ViewButton,
    args: { item: { label: 'Friends', view: '/friends' } }
  });
</script>

<Story name="Default" />
<Story name="Current" args={{ current: true }} />
<Story
  name="With badge"
  args={{ item: { label: 'Notifications', view: '/notifications', badge: 12 } }}
/>

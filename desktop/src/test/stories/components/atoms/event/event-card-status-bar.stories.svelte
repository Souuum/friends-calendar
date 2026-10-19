<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventCardStatusBar from '$lib/components/atoms/event/EventCardStatusBar.svelte';

  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/EventCardStatusBar',
    component: EventCardStatusBar,
    argTypes: {
      status: { control: 'inline-radio', options: ['pending', 'accepted', 'maybe', 'declined'] }
    },
    args: { status: 'pending' }
  });
</script>

<Story name="Pending" />
<Story name="Accepted" args={{ status: 'accepted' }} />
<Story name="Maybe" args={{ status: 'maybe' }} />
<Story name="Declined" args={{ status: 'declined' }} />

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventCardParticipant from '$lib/components/atoms/event/EventCardParticipant.svelte';
  import { PARTICIPANTS } from '../../../fixtures';

  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/EventCardParticipant',
    component: EventCardParticipant,
    args: { participants: PARTICIPANTS }
  });
</script>

<Story name="Mixed statuses" />
<Story name="One participant" args={{ participants: PARTICIPANTS.slice(0, 1) }} />
<Story
  name="Overflowing"
  args={{
    participants: [...PARTICIPANTS, ...PARTICIPANTS].map((p, i) => ({
      ...p,
      user_id: `${p.user_id}-${i}`
    }))
  }}
/>

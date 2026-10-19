<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import InviteFriendSheet from '$lib/components/organisms/InviteFriendSheet.svelte';
  import { BEST_SLOT, EVENT_HOSTED, EVENT_LONG_TITLE, FRIENDS } from '../../fixtures';
  import { mockApi, delay, pending } from '../../mockApi';

  const { Story } = defineMeta({
    title: 'Components/Organisms/InviteFriendSheet',
    component: InviteFriendSheet,
    parameters: { layout: 'fullscreen' },
    args: { friend: FRIENDS[0] },
    beforeEach: () =>
      mockApi({
        getInvitableEvents: async () => [EVENT_HOSTED, EVENT_LONG_TITLE],
        getBestSlots: async () => [BEST_SLOT],
        inviteParticipants: () => delay({})
      })
  });
</script>

<Story name="Events to invite to" />
<Story
  name="Nothing to invite to"
  beforeEach={() => mockApi({ getInvitableEvents: async () => [], getBestSlots: async () => [] })}
/>
<Story
  name="Loading"
  beforeEach={() => mockApi({ getInvitableEvents: () => pending(), getBestSlots: () => pending() })}
/>

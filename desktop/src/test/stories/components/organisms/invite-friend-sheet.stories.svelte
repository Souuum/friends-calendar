<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import InviteFriendSheet from '$lib/components/organisms/InviteFriendSheet.svelte';
  import { BEST_SLOT, EVENT_HOSTED, EVENT_LONG_TITLE, FRIENDS } from '../../fixtures';
  import { mockApi, delay, pending } from '../../mockApi';

  /**
   * The sheet behind "Invite" on a friend's profile: pick one of your upcoming
   * events to add the friend to, or start a new event with them. Mount it only
   * while it is open.
   *
   * ```svelte
   * {#if inviting && friend}
   *   <InviteFriendSheet
   *     {friend}
   *     on:close={() => (inviting = false)}
   *     on:invited={handleInvited}
   *     on:createWith={handleCreateWith}
   *   />
   * {/if}
   * ```
   *
   * - On mount it fetches `api.getInvitableEvents(friend.user_id)` (events you
   *   created that the friend is not on yet) and, best-effort,
   *   `api.getBestSlots()` for the next 7 days.
   * - Dispatches `invited` with `{ event }` after `api.inviteParticipants()`
   *   succeeds, `createWith` with `{ start }` (the best shared slot, or `null`)
   *   from "New event with ...", and `close` (× button, Escape, backdrop tap,
   *   back gesture). It never unmounts itself.
   * - Renders a `position: fixed` full-screen overlay.
   */
  const { Story } = defineMeta({
    title: 'Components/Organisms/InviteFriendSheet',
    component: InviteFriendSheet,
    parameters: {
      layout: 'fullscreen',
      docs: { story: { inline: false, height: '560px' } }
    },
    args: { friend: FRIENDS[0] },
    beforeEach: () =>
      mockApi({
        getInvitableEvents: async () => [EVENT_HOSTED, EVENT_LONG_TITLE],
        getBestSlots: async () => [BEST_SLOT],
        inviteParticipants: () => delay({})
      })
  });
</script>

<!-- A `friend` with two events they can be added to, and a shared free slot under "New event with ...". -->
<Story name="Events to invite to" />

<!-- No invitable event and no shared slot: only the "New event with ..." action is left. -->
<Story
  name="Nothing to invite to"
  beforeEach={() => mockApi({ getInvitableEvents: async () => [], getBestSlots: async () => [] })}
/>

<!-- The events request never resolves: the loading line stays up. -->
<Story
  name="Loading"
  beforeEach={() => mockApi({ getInvitableEvents: () => pending(), getBestSlots: () => pending() })}
/>

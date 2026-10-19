<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Calendar from '$lib/components/templates/Calendar.svelte';
  import { user } from '$lib/stores';
  import { BEST_SLOT, BUSY, EVENTS, FRIENDS, GUILDS, INVITE_URL, ME } from '../../fixtures';
  import { mockApi } from '../../mockApi';

  const { Story } = defineMeta({
    title: 'Components/Templates/Calendar',
    component: Calendar,
    parameters: { layout: 'fullscreen' },
    beforeEach: () => {
      user.set(ME);
      const restore = mockApi({
        getFreeFriendsNow: async () => ['u1', 'u2'],
        getFriends: async () => FRIENDS,
        getExternalBusy: async () => BUSY,
        getBestSlots: async () => [BEST_SLOT],
        getServers: async () => ({ guilds: GUILDS, invite_url: INVITE_URL })
      });
      return () => {
        restore();
        user.set(null);
      };
    }
  });
</script>

<!-- `events` collides with a Svelte mount option, so it goes through a template. -->
{#snippet withEvents()}
  <Calendar events={EVENTS} />
{/snippet}

<Story name="With events" template={withEvents} />
<Story name="Empty" />

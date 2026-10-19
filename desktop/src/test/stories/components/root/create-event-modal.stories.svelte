<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CreateEventModal from '$lib/components/CreateEventModal.svelte';
  import { user } from '$lib/stores';
  import { EVENT_HOSTED, FRIENDS, GUILDS, INVITE_URL, ME, at } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  const { Story } = defineMeta({
    title: 'Components/CreateEventModal',
    component: CreateEventModal,
    parameters: { layout: 'fullscreen' },
    beforeEach: () => {
      user.set(ME);
      const restore = mockApi({
        getFriends: async () => FRIENDS,
        getServers: async () => ({ guilds: GUILDS, invite_url: INVITE_URL }),
        previewAnnouncement: async () => '## Soirée jeux de société\n> Lieu : **Chez Hugo**',
        createEvent: () => delay(EVENT_HOSTED),
        updateEvent: () => delay(EVENT_HOSTED)
      });
      return () => {
        restore();
        user.set(null);
      };
    }
  });
</script>

<Story name="Create" />
<Story name="Create on a chosen day" args={{ initialDate: new Date(at(3, 0)) }} />
<Story name="Create with a friend preselected" args={{ initialParticipantIds: ['u1'] }} />
<Story name="Edit" args={{ event: EVENT_HOSTED }} />

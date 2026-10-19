<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import AdoptEventModal from '$lib/components/organisms/AdoptEventModal.svelte';
  import { user } from '$lib/stores';
  import { ANNOUNCEMENT_BOT, ANNOUNCEMENT_HANDWRITTEN, EVENT_HOSTED, ME } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  const { Story } = defineMeta({
    title: 'Components/Organisms/AdoptEventModal',
    component: AdoptEventModal,
    parameters: { layout: 'fullscreen' },
    args: { post: ANNOUNCEMENT_BOT },
    beforeEach: () => {
      user.set(ME);
      const restore = mockApi({
        adoptAnnouncement: () =>
          delay({ event: EVENT_HOSTED, rsvps_recorded: 4, backfill_failed: false })
      });
      return () => {
        restore();
        user.set(null);
      };
    }
  });
</script>

<!-- The bot's own template parses cleanly, so the form arrives prefilled. -->
<Story name="Parsed announcement" />
<!-- No <t:…> timestamp: the form names what it couldn't read. -->
<Story name="Unparseable post" args={{ post: ANNOUNCEMENT_HANDWRITTEN }} />

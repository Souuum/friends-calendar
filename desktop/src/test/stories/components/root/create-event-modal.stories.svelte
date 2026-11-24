<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CreateEventModal from '$lib/components/CreateEventModal.svelte';
  import { user } from '$lib/stores';
  import { EVENT_HOSTED, FRIENDS, GUILDS, INVITE_URL, ME, at } from '../../fixtures';
  import { mockApi, delay } from '../../mockApi';

  /**
   * The event form, as a modal: creates a new event, or edits one when `event` is set. The
   * parent decides when it is open by mounting it, and unmounts it on `close` or `saved`.
   *
   * ```svelte
   * {#if showCreateModal}
   *   <CreateEventModal
   *     event={editingEvent}
   *     {initialDate}
   *     on:close={() => (showCreateModal = false)}
   *     on:saved={reloadEvents}
   *   />
   * {/if}
   * ```
   *
   * - Dispatches `saved` once `api.createEvent()` or `api.updateEvent()` has succeeded, and
   *   `close` from the × button, Cancel, Escape, a tap on the backdrop or the back gesture.
   *   Neither carries a payload, and the modal does not close itself.
   * - Renders a `position: fixed` full-screen overlay. No slot.
   * - Reads the `user` store to preselect the visibility from `default_visibility`.
   * - On mount it calls `api.getFriends()` for the invite picker and `api.getServers()` for
   *   the "Announce in a server" chips.
   * - Creating is a two-step wizard below `md:` (768px), where step 2 calls
   *   `api.previewAnnouncement()`; from `md:` up it is one scrolling form.
   * - Editing has no invite picker, no server choice and no wizard: the guest list and where
   *   an event was announced are not changed from here.
   */
  const { Story } = defineMeta({
    title: 'Components/CreateEventModal',
    component: CreateEventModal,
    parameters: {
      layout: 'fullscreen',
      // A fixed full-screen overlay, and every story stubs `api` and sets the `user` store,
      // so on the docs page each story needs a frame of its own.
      docs: { story: { inline: false, height: '760px' } }
    },
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

<!-- No props: an empty form for a new event, with the invite picker and the announce choice. -->
<Story name="Create" />

<!-- `initialDate` set: the start and end times open on that day, as after a long-press on the grid. -->
<Story name="Create on a chosen day" args={{ initialDate: new Date(at(3, 0)) }} />

<!-- `initialParticipantIds` set: that friend starts ticked in the invite picker, and others can be added. -->
<Story name="Create with a friend preselected" args={{ initialParticipantIds: ['u1'] }} />

<!-- `event` set: the form opens filled in as "Edit event", without the invite picker or the server choice. -->
<Story name="Edit" args={{ event: EVENT_HOSTED }} />

<!-- `initialDate` and `initialParticipantIds` together, as the friend profile's "New event with..." passes them. -->
<Story
  name="Create with a friend on a chosen day"
  args={{ initialDate: new Date(at(2, 20)), initialParticipantIds: ['u1'] }}
/>

<!-- Below 768px creating is a two-step wizard ("1 / 2"); the canvas opens on a mobile viewport. -->
<Story name="Create on mobile" globals={{ viewport: { value: 'mobile1', isRotated: false } }} />

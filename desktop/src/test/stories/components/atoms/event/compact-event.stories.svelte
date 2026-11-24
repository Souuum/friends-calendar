<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import CompactEvent from '$lib/components/atoms/event/CompactEvent.svelte';
  import {
    EVENT_HOSTED,
    EVENT_INVITED,
    EVENT_MAYBE,
    EVENT_DECLINED,
    EVENT_DISCOVERED,
    EVENT_LONG_TITLE
  } from '../../../fixtures';

  const EVENT_BARE = { ...EVENT_HOSTED, title: '', participants: [] };

  /**
   * The one-line event chip of the month grid: a 3px rail coloured by your
   * answer, the title, and the number of participants.
   *
   * ```svelte
   * <button type="button" class="w-full" on:click={() => select(event)}>
   *   <CompactEvent {event} />
   * </button>
   * ```
   *
   * It is not interactive and forwards no events, so the caller wraps it in
   * whatever should react to a click. It fills the width of its container and
   * truncates the title. An event you can see but are not invited to
   * (`is_participant: false`) is muted and gets a tooltip saying so.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/CompactEvent',
    component: CompactEvent,
    args: { event: EVENT_HOSTED },
    parameters: { layout: 'padded' }
  });
</script>

<!-- An `event` with `my_status: 'accepted'`: the rail takes the "going" colour. -->
<Story name="Going" />

<!-- `my_status: 'pending'`: an invitation not answered yet. -->
<Story name="Awaiting answer" args={{ event: EVENT_INVITED }} />

<!-- `my_status: 'maybe'`. -->
<Story name="Maybe" args={{ event: EVENT_MAYBE }} />

<!-- `my_status: 'declined'`: the title is muted and struck through. -->
<Story name="Declined" args={{ event: EVENT_DECLINED }} />

<!-- `is_participant: false`: a grey rail, a muted title and a tooltip on hover. -->
<Story name="Discovered (not invited)" args={{ event: EVENT_DISCOVERED }} />

<!-- An empty `title` falls back to "Untitled Event", and no headcount is shown without participants. -->
<Story name="Untitled, nobody invited" args={{ event: EVENT_BARE }} />

<!-- A long title in a 128px-wide container is cut off with an ellipsis. -->
<Story name="Long title in a narrow cell" asChild>
  <div class="w-32">
    <CompactEvent event={EVENT_LONG_TITLE} />
  </div>
</Story>

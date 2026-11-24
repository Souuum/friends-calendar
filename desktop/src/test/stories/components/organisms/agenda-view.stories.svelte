<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import AgendaView from '$lib/components/organisms/AgendaView.svelte';
  import { EVENTS } from '../../fixtures';

  /**
   * The calendar's list view: upcoming events grouped under a heading per day,
   * soonest first. It is the view that stays readable on a phone, where a
   * seven-column grid is too narrow.
   *
   * ```svelte
   * <AgendaView events={filteredEvents} onEventClick={selectEvent} />
   * ```
   *
   * - Events that have already started are dropped, and days are local days.
   * - With nothing left to show it renders "Nothing coming up."
   * - Events the viewer is not a participant of get a dashed border and a
   *   "Not invited" label.
   * - Dispatches no events: a tapped row calls `onEventClick` with the event.
   */
  const { Story } = defineMeta({
    title: 'Components/Organisms/AgendaView',
    component: AgendaView,
    args: { events: EVENTS, onEventClick: fn() }
  });
</script>

<!-- `events` holding upcoming events: one group per day, with status rail, time, place and headcount. -->
<Story name="Upcoming events">
  {#snippet template(args)}
    <AgendaView {...args} />
  {/snippet}
</Story>

<!-- An empty `events` list shows the "Nothing coming up." message. -->
<Story name="Empty" args={{ events: [] }}>
  {#snippet template(args)}
    <AgendaView {...args} />
  {/snippet}
</Story>

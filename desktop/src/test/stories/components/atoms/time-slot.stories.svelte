<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import TimeSlot from '$lib/components/atoms/TimeSlot.svelte';

  /**
   * One hour cell of the day and week grids: an 80px-tall bordered box whose
   * default slot holds that hour's events.
   *
   * ```svelte
   * <TimeSlot {hour} hasEvents={hourEvents.length > 0}>
   *   {#each hourEvents as event (event.id)}
   *     <TimedEvent {event} onClick={() => select(event)} />
   *   {/each}
   * </TimeSlot>
   * ```
   *
   * The cell is positioned (`relative`), so slot content can be placed
   * absolutely inside it. `hour` is accepted but not displayed: the label
   * beside the cell is a separate `TimeLabel`.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/TimeSlot',
    component: TimeSlot,
    args: { hour: 9 }
  });
</script>

<!-- The default `hasEvents={false}`: a plain cell. -->
<Story name="Empty" />

<!-- `hasEvents` tints the cell. -->
<Story name="Has events" args={{ hasEvents: true }} />

<!-- Slot content drawn inside the cell, with `hasEvents` set as the grids do. -->
<Story name="With content" args={{ hasEvents: true }}>
  {#snippet template(args)}
    <TimeSlot {...args} hour={'hour' in args ? args.hour : 9}>
      <div class="m-1 rounded bg-primary px-2 py-1 text-xs text-white">Soirée jeux de société</div>
    </TimeSlot>
  {/snippet}
</Story>

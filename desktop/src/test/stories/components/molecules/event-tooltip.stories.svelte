<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import type { ComponentProps } from 'svelte';
  import EventTooltip from '$lib/components/molecules/EventTooltip.svelte';
  import { EVENT_HOSTED, EVENT_INVITED } from '../../fixtures';

  /**
   * A floating panel with one full event card per event. Use it to show a day's
   * events next to the cell being hovered.
   *
   * ```svelte
   * <EventTooltip
   *   events={tooltipEvents}
   *   isVisible={tooltipVisible}
   *   position={tooltipPosition}
   *   on:mouseenter={keepOpen}
   *   on:mouseleave={hide}
   *   on:refresh={reload}
   * />
   * ```
   *
   * - Renders `position: fixed` at `position`, in viewport pixels. The caller
   *   works out where and keeps it on screen.
   * - Dispatches `mouseenter` and `mouseleave` so the caller can keep it open
   *   while the pointer is over it, and `refresh` when a card changed the event.
   * - The cards are interactive: answering or deleting calls the API from inside
   *   the card.
   * - Renders nothing while `isVisible` is off or `events` is empty.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/EventTooltip',
    component: EventTooltip,
    args: { events: [EVENT_HOSTED], isVisible: true, position: { x: 24, y: 24 } },
    // `position: fixed`: without a frame each, every story would stack at the same spot on the docs page.
    parameters: { docs: { story: { inline: false, height: '560px' } } }
  });
</script>

<!-- Through a snippet: a prop named `events` collides with a Svelte mount option. -->
{#snippet tooltip(args: ComponentProps<typeof EventTooltip>)}
  <EventTooltip {...args} />
{/snippet}

<!-- One event, placed 24px from the top-left corner by `position`. -->
<Story name="One event" template={tooltip} />

<!-- Two events stack as two cards; the panel scrolls past 500px. -->
<Story name="Several events" args={{ events: [EVENT_HOSTED, EVENT_INVITED] }} template={tooltip} />

<!-- `isVisible={false}` renders nothing. -->
<Story
  name="Hidden"
  args={{ isVisible: false }}
  parameters={{ docs: { story: { height: '60px' } } }}
  template={tooltip}
/>

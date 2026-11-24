<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import type { ComponentProps } from 'svelte';
  import EventRsvpCard from '$lib/components/molecules/EventRsvpCard.svelte';
  import {
    EVENT_HOSTED,
    EVENT_INVITED,
    EVENT_MAYBE,
    EVENT_DECLINED,
    EVENT_LONG_TITLE
  } from '../../fixtures';

  /**
   * A read-only card for one event: title, your RSVP as a badge, then the
   * description, date, location, price, link and participants that the event
   * has. Use it to list events where answering is not the point, such as
   * "Shared events" on a friend's page.
   *
   * ```svelte
   * {#each sharedEvents as event (event.id)}
   *   <EventRsvpCard {event} />
   * {/each}
   * ```
   *
   * - The badge follows `event.my_status`: "You accepted", "You said maybe",
   *   "You declined", or "No response yet" for anything else.
   * - It has no controls and dispatches nothing. Changing an RSVP happens in
   *   `EventPeekPanel`.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/EventRsvpCard',
    component: EventRsvpCard,
    args: { event: EVENT_HOSTED }
  });
</script>

{#snippet narrow(args: ComponentProps<typeof EventRsvpCard>)}
  <div class="w-[360px]">
    <EventRsvpCard {...args} />
  </div>
{/snippet}

<!-- An accepted event with every optional field set: description, location, price and link. -->
<Story name="Going, with price and link" />

<!-- `my_status` is `pending`: the badge reads "No response yet". -->
<Story name="Awaiting answer" args={{ event: EVENT_INVITED }} />

<!-- `my_status` is `maybe`. -->
<Story name="Maybe" args={{ event: EVENT_MAYBE }} />

<!-- `my_status` is `declined`. -->
<Story name="Declined" args={{ event: EVENT_DECLINED }} />

<!-- A long title and an unbroken URL in a 360px column: this once widened /friends/[id] past the viewport. -->
<Story
  name="Narrow, long content"
  args={{
    event: {
      ...EVENT_LONG_TITLE,
      link: 'https://example.com/a/rather/long/link/that/should/not/overflow'
    }
  }}
  template={narrow}
/>

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import EventCardParticipant from '$lib/components/atoms/event/EventCardParticipant.svelte';
  import { PARTICIPANTS } from '../../../fixtures';

  /**
   * The guest list of an event card: a count, then up to five people, each
   * with an avatar and a name tinted by their answer.
   *
   * ```svelte
   * <EventCardParticipant participants={event.participants} />
   * ```
   *
   * Beyond five people the rest are summed up as "+N more". Someone without
   * an `avatar_url` gets a plain grey circle. Green is accepted, yellow is
   * maybe, red is declined and grey is no answer yet.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/Event/EventCardParticipant',
    component: EventCardParticipant,
    args: { participants: PARTICIPANTS }
  });
</script>

<!-- Four `participants`, one per answer, so every tint shows. -->
<Story name="Mixed statuses" />

<!-- A single participant: the count reads "1 participant", in the singular. -->
<Story name="One participant" args={{ participants: PARTICIPANTS.slice(0, 1) }} />

<!-- Eight `participants`: five are listed and the rest become "+3 more". -->
<Story
  name="Overflowing"
  args={{
    participants: [...PARTICIPANTS, ...PARTICIPANTS].map((p, i) => ({
      ...p,
      user_id: `${p.user_id}-${i}`
    }))
  }}
/>

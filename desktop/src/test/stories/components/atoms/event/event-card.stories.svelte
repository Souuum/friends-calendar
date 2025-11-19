<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
    import EventCard from '$lib/components/atoms/event/EventCard.svelte';
  import { EVENT_CREATOR, EVENT_PARTICIPANT_ACCEPTED, EVENT_PARTICIPANTS_5, EVENT_PARTICIPANTS_6 } from './mock';

  const { Story } = defineMeta({
    title: 'Components/Atoms/EventCard',
    component: EventCard
  });


</script>

<Story name="Default Creator" args={{event: EVENT_CREATOR}}/>

<Story name="Default Participant" args={{event: EVENT_PARTICIPANT_ACCEPTED}}/>

<Story name="Default Participant 5" args={{event: EVENT_PARTICIPANTS_5}}/>

<Story name="Default participant 6" args={{event: EVENT_PARTICIPANTS_6}}/>
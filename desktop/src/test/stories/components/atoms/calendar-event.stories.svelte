<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
    import CalendarEvent from '$lib/components/atoms/CalendarEvent.svelte';
  import { EVENT_CREATOR } from './event/mock';
    const { Story } = defineMeta({
        title: 'Components/Atoms/CalendarEvent',
        component: CalendarEvent
    });
</script>

<Story name="Default" args={{event: EVENT_CREATOR}}/>
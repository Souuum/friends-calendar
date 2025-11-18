<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
    import CalendarDay from '$lib/components/atoms/CalendarDay.svelte';

    const { Story } = defineMeta({
        title: 'Components/Atoms/CalendarDay',
        component: CalendarDay
    });
</script>

<Story name="Large date" args={{
    date: new Date('2024-06-15'),
    isCurrentMonth: true,
    isToday: false,
    size: 'large'
}}/>

<Story name="Large today" args={{
    date: new Date('2024-06-15'),
    isCurrentMonth: true,
    isToday: true,
    size: 'large'
}}/>

<Story name="Small date" args={{
    date: new Date('2024-06-15'),
    isCurrentMonth: true,
    isToday: false,
    size: 'small'
}}/>

<Story name="Small today date" args={{
    date: new Date('2024-06-15'),
    isCurrentMonth: true,
    isToday: true,
    size: 'small'
}}/>
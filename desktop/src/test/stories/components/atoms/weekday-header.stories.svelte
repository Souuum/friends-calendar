<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
    import WeekdayHeader from '$lib/components/atoms/WeekdayHeader.svelte';

    const { Story } = defineMeta({
        title: 'Components/Atoms/WeekdayHeader',
        component: WeekdayHeader
    });
</script>

<Story name="Default" />
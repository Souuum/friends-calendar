<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
    import ArrowButton from '$lib/components/atoms/ArrowButton.svelte';

    const { Story } = defineMeta({
        title: 'Components/Atoms/ArrowButton',
        component: ArrowButton
    });
</script>

<Story name="Default" />

<Story name="Right" args={{direction: 'right'}}/>
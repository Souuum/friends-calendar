<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
    import BlurOverlay from '$lib/components/atoms/BlurOverlay.svelte';

    const { Story } = defineMeta({
        title: 'Components/Atoms/BlurOverlay',
        component: BlurOverlay
    });
</script>

<Story name="Default">
    {#snippet template({...args})}
        <BlurOverlay {...args}>
            <div class="m-4 p-4">
                <pre>Hello World</pre>
            </div>
        </BlurOverlay>
    {/snippet}
</Story>
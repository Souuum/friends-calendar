<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Button from '$lib/components/atoms/Button.svelte';

  const { Story } = defineMeta({
    title: 'Components/Atoms/Button',
    component: Button
  });
</script>

<Story name="Default">
  My button
</Story>

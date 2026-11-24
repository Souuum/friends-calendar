<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Button from '$lib/components/atoms/Button.svelte';

  /**
   * A filled call-to-action button. The label goes in the default slot.
   *
   * ```svelte
   * <form on:submit|preventDefault={save}>
   *   <Button type="submit">Save changes</Button>
   * </form>
   * ```
   *
   * It forwards no events (there is no `on:click`), so it only does something
   * as the submit or reset button of a surrounding `<form>`.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/Button',
    component: Button
  });
</script>

<!-- The defaults: `variant="primary"`, `type="button"`. -->
<Story name="Default">
  {#snippet template(args)}
    <Button {...args}>My button</Button>
  {/snippet}
</Story>

<!-- `type="submit"` looks the same; the difference is that a surrounding form submits. -->
<Story name="Submit" args={{ type: 'submit' }}>
  {#snippet template(args)}
    <Button {...args}>Save changes</Button>
  {/snippet}
</Story>

<!-- Any `variant` other than `primary` drops the fill and the text colour. -->
<Story name="Unstyled variant" args={{ variant: 'plain' }}>
  {#snippet template(args)}
    <Button {...args}>Plain</Button>
  {/snippet}
</Story>

<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import type { ComponentProps } from 'svelte';
  import ModalContainer from '$lib/components/molecules/ModalContainer.svelte';

  const { Story } = defineMeta({
    title: 'Components/Molecules/ModalContainer',
    component: ModalContainer,
    argTypes: {
      size: { control: 'inline-radio', options: ['sm', 'md', 'lg', 'xl', 'full'] },
      position: { control: 'inline-radio', options: ['center', 'top', 'bottom'] }
    },
    args: { isOpen: true, onClose: fn() }
  });
</script>

{#snippet withContent(args: ComponentProps<typeof ModalContainer>)}
  <ModalContainer {...args}>
    <div class="p-6">
      <h2 class="text-lg font-semibold">Modal title</h2>
      <p class="mt-2 text-sm text-muted">Some modal content.</p>
    </div>
  </ModalContainer>
{/snippet}

<Story name="Medium" template={withContent} />
<Story name="Small" args={{ size: 'sm' }} template={withContent} />
<Story name="Large, top" args={{ size: 'lg', position: 'top' }} template={withContent} />

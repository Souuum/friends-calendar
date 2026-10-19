<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import type { ComponentProps } from 'svelte';
  import BlurModal from '$lib/components/organisms/BlurModal.svelte';

  const { Story } = defineMeta({
    title: 'Components/Organisms/BlurModal',
    component: BlurModal,
    parameters: { layout: 'fullscreen' },
    argTypes: {
      blurAmount: { control: 'inline-radio', options: ['sm', 'md', 'lg'] },
      overlayOpacity: { control: 'inline-radio', options: ['light', 'medium', 'dark'] }
    },
    args: { isOpen: true, onClose: fn() }
  });
</script>

{#snippet withContent(args: ComponentProps<typeof BlurModal>)}
  <p class="p-6">Content behind the overlay.</p>
  <BlurModal {...args}>
    <div class="p-6">
      <h2 class="text-lg font-semibold">Blurred modal</h2>
      <p class="mt-2 text-sm text-muted">Not used by any screen today; kept for reference.</p>
    </div>
  </BlurModal>
{/snippet}

<Story name="Default" template={withContent} />
<Story
  name="Heavy blur, dark"
  args={{ blurAmount: 'lg', overlayOpacity: 'dark' }}
  template={withContent}
/>

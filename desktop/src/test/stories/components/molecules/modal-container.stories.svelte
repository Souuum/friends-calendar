<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import type { ComponentProps } from 'svelte';
  import ModalContainer from '$lib/components/molecules/ModalContainer.svelte';

  /**
   * A dialog panel laid over the whole viewport, with the content in its default
   * slot. It is the positioning half of `BlurModal`, which adds the backdrop.
   *
   * ```svelte
   * <ModalContainer isOpen={showDetails} size="sm" onClose={() => (showDetails = false)}>
   *   <div class="p-6">
   *     <h2>Leave this event?</h2>
   *   </div>
   * </ModalContainer>
   * ```
   *
   * - Renders `position: fixed` over the viewport, with no backdrop colour of its
   *   own.
   * - It never closes itself: a click outside the panel or Escape calls
   *   `onClose`, and the caller sets `isOpen` to `false`. Without `onClose`
   *   neither does anything.
   * - Escape is listened for on `window`.
   * - The panel is at most 90vh tall and clips what overflows, so long content
   *   needs its own scroll container.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/ModalContainer',
    component: ModalContainer,
    argTypes: {
      size: { control: 'inline-radio', options: ['sm', 'md', 'lg', 'xl', 'full'] },
      position: { control: 'inline-radio', options: ['center', 'top', 'bottom'] }
    },
    args: { isOpen: true, onClose: fn() },
    // `position: fixed`: without a frame each, every story would cover the docs page.
    parameters: { docs: { story: { inline: false, height: '360px' } } }
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

<!-- The defaults: `size="md"`, centred. Click outside the panel or press Escape to see `onClose` in the Actions panel. -->
<Story name="Medium" template={withContent} />

<!-- `size="sm"`, the narrowest panel. -->
<Story name="Small" args={{ size: 'sm' }} template={withContent} />

<!-- `size="lg"` with `position="top"`: the panel sits 80px below the top edge. -->
<Story name="Large, top" args={{ size: 'lg', position: 'top' }} template={withContent} />

<!-- `size="full"` with `position="bottom"`: full width, 80px above the bottom edge. -->
<Story
  name="Full width, bottom"
  args={{ size: 'full', position: 'bottom' }}
  template={withContent}
/>

<!-- `closeOnBackdrop={false}`: a click outside the panel no longer calls `onClose`; Escape still does. -->
<Story name="Backdrop click ignored" args={{ closeOnBackdrop: false }} template={withContent} />

<!-- `closeOnEscape={false}`: Escape no longer calls `onClose`; a click outside still does. -->
<Story name="Escape ignored" args={{ closeOnEscape: false }} template={withContent} />

<!-- `isOpen={false}`: nothing is rendered. -->
<Story
  name="Closed"
  args={{ isOpen: false }}
  parameters={{ docs: { story: { height: '60px' } } }}
  template={withContent}
/>

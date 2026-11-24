<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import { fn } from 'storybook/test';
  import type { ComponentProps } from 'svelte';
  import BlurModal from '$lib/components/organisms/BlurModal.svelte';

  /**
   * A generic modal: a blurred, dimmed overlay with a card on top of it. The
   * card's content goes in the default slot. No screen uses it today - the
   * app's real dialogs build their own overlay.
   *
   * ```svelte
   * <BlurModal isOpen={showDetails} size="sm" onClose={() => (showDetails = false)}>
   *   <div class="p-6">
   *     <h2 class="text-lg font-semibold">Game night</h2>
   *     <p class="mt-2 text-sm text-muted">Friday 20:00, at Mimo's.</p>
   *   </div>
   * </BlurModal>
   * ```
   *
   * - Renders nothing while `isOpen` is false, and never closes itself: the
   *   caller flips `isOpen` from `onClose`.
   * - Dispatches no events. `onClose` is a callback prop, called on a backdrop
   *   click (`closeOnBackdrop`) and on Escape (`closeOnEscape`).
   * - Both layers are `position: fixed` and cover the viewport.
   */
  const { Story } = defineMeta({
    title: 'Components/Organisms/BlurModal',
    component: BlurModal,
    parameters: {
      layout: 'fullscreen',
      docs: { story: { inline: false, height: '480px' } }
    },
    argTypes: {
      size: { control: 'inline-radio', options: ['sm', 'md', 'lg', 'xl', 'full'] },
      position: { control: 'inline-radio', options: ['center', 'top', 'bottom'] },
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

<!-- The defaults: `size="md"`, `position="center"`, medium blur and medium dimming. -->
<Story name="Default" template={withContent} />

<!-- `blurAmount="lg"` with `overlayOpacity="dark"`: the strongest overlay. -->
<Story
  name="Heavy blur, dark"
  args={{ blurAmount: 'lg', overlayOpacity: 'dark' }}
  template={withContent}
/>

<!-- `size="sm"`: the narrowest card. -->
<Story name="Small" args={{ size: 'sm' }} template={withContent} />

<!-- `size="xl"`: the widest capped card. -->
<Story name="Extra large" args={{ size: 'xl' }} template={withContent} />

<!-- `size="full"`: the card takes the full width, less a side margin. -->
<Story name="Full width" args={{ size: 'full' }} template={withContent} />

<!-- `position="top"`: the card sits near the top of the viewport instead of the middle. -->
<Story name="Top" args={{ position: 'top' }} template={withContent} />

<!-- `position="bottom"`: the card sits near the bottom of the viewport. -->
<Story name="Bottom" args={{ position: 'bottom' }} template={withContent} />

<!-- `closeOnBackdrop={false}`: `closeOnBackdrop` no longer routes a click outside the card to `onClose`. -->
<Story name="Backdrop click ignored" args={{ closeOnBackdrop: false }} template={withContent} />

<!-- `closeOnEscape={false}`: pressing Escape does not call `onClose`. -->
<Story name="Escape ignored" args={{ closeOnEscape: false }} template={withContent} />

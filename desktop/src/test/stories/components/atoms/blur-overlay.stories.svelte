<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import type { ComponentProps } from 'svelte';
  import BlurOverlay from '$lib/components/atoms/BlurOverlay.svelte';

  /**
   * A full-viewport scrim that blurs and tints the page behind it. It is the
   * backdrop of `BlurModal`: the dialog goes in the default slot.
   *
   * ```svelte
   * <BlurOverlay visible={isOpen} blurAmount="md" opacity="dark" on:click={close}>
   *   <ModalContainer {isOpen} onClose={close}>Are you sure?</ModalContainer>
   * </BlurOverlay>
   * ```
   *
   * It renders `position: fixed` over the whole viewport, and nothing at all
   * while `visible` is false. It forwards `on:click`; a click on the slot
   * content bubbles up to it too, so the caller decides which clicks close.
   * There is no keyboard handling.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/BlurOverlay',
    component: BlurOverlay,
    argTypes: {
      blurAmount: { control: 'inline-radio', options: ['sm', 'md', 'lg'] },
      opacity: { control: 'inline-radio', options: ['light', 'medium', 'dark'] }
    },
    args: { visible: true },
    // The overlay is fixed to the viewport, so on the docs page each story
    // needs a frame of its own or the first one covers everything.
    parameters: { docs: { story: { inline: false, height: '240px' } } }
  });
</script>

{#snippet overPage(args: ComponentProps<typeof BlurOverlay>)}
  <div class="p-6 text-ink">
    <h2 class="text-lg font-semibold">Page content</h2>
    <p class="text-sm text-muted">
      This text sits under the overlay, so the blur and the tint show on it.
    </p>
  </div>
  <BlurOverlay {...args}>
    <div class="mx-6 mt-24 rounded-lg bg-surface p-4 text-sm text-ink shadow-lg">
      Slot content, drawn above the scrim.
    </div>
  </BlurOverlay>
{/snippet}

<!-- `visible` with the defaults: `blurAmount="sm"` and `opacity="medium"`. -->
<Story name="Default" template={overPage} />

<!-- `blurAmount="md"`: a stronger blur of the page behind. -->
<Story name="Medium blur" args={{ blurAmount: 'md' }} template={overPage} />

<!-- `blurAmount="lg"`: the strongest blur. -->
<Story name="Large blur" args={{ blurAmount: 'lg' }} template={overPage} />

<!-- `opacity="light"`: the weakest tint, so the page stays readable. -->
<Story name="Light tint" args={{ opacity: 'light' }} template={overPage} />

<!-- `opacity="dark"`: the strongest tint. -->
<Story name="Dark tint" args={{ opacity: 'dark' }} template={overPage} />

<!-- `visible={false}`: neither the scrim nor the slot content is rendered. -->
<Story name="Hidden" args={{ visible: false }} template={overPage} />

<script lang="ts">
  import BlurOverlay from '$lib/components/atoms/BlurOverlay.svelte';
  import ModalContainer from '$lib/components/molecules/ModalContainer.svelte';

  /** Whether the modal is shown. `false` (the default) renders nothing at all. */
  export let isOpen = false;
  /** Maximum width of the dialog card: `sm` 28rem, `md` 42rem, `lg` 56rem, `xl` 72rem, `full` the viewport width minus a margin. */
  export let size: 'sm' | 'md' | 'lg' | 'xl' | 'full' = 'md';
  /** Where the card sits vertically: `center`, or `top` / `bottom` with a 5rem gap from that edge. */
  export let position: 'center' | 'top' | 'bottom' = 'center';
  /** Called when the user asks to close (backdrop click or Escape). The modal never closes itself: set `isOpen` to `false` here. */
  export let onClose: (() => void) | undefined = undefined;
  /** Whether a click outside the card calls `onClose`. */
  export let closeOnBackdrop = true;
  /** Whether pressing Escape while open calls `onClose`. */
  export let closeOnEscape = true;
  /** How strongly the page behind is blurred: `sm`, `md` or `lg`. */
  export let blurAmount: 'sm' | 'md' | 'lg' = 'sm';
  /** How dark the tint over the page is: `light` 30%, `medium` 50%, `dark` 70%. */
  export let overlayOpacity: 'light' | 'medium' | 'dark' = 'medium';

  function handleBlurClick() {
    if (closeOnBackdrop && onClose) {
      onClose();
    }
  }
</script>

{#if isOpen}
  <BlurOverlay visible={isOpen} {blurAmount} opacity={overlayOpacity} on:click={handleBlurClick}>
    <ModalContainer {isOpen} {size} {position} {onClose} {closeOnBackdrop} {closeOnEscape}>
      <slot />
    </ModalContainer>
  </BlurOverlay>
{/if}

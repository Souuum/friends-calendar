<script lang="ts">
  import BlurOverlay from '$lib/components/atoms/BlurOverlay.svelte';
  import ModalContainer from '$lib/components/molecules/ModalContainer.svelte';

  export let isOpen = false;
  export let size: 'sm' | 'md' | 'lg' | 'xl' | 'full' = 'md';
  export let position: 'center' | 'top' | 'bottom' = 'center';
  export let onClose: (() => void) | undefined = undefined;
  export let closeOnBackdrop = true;
  export let closeOnEscape = true;
  export let blurAmount: 'sm' | 'md' | 'lg' = 'sm';
  export let overlayOpacity: 'light' | 'medium' | 'dark' = 'medium';

  function handleBlurClick() {
    if (closeOnBackdrop && onClose) {
      onClose();
    }
  }
</script>

{#if isOpen}
  <BlurOverlay
    visible={isOpen}
    {blurAmount}
    opacity={overlayOpacity}
    on:click={handleBlurClick}
  >
    <ModalContainer {isOpen} {size} {position} {onClose} {closeOnBackdrop} {closeOnEscape}>
      <slot />
    </ModalContainer>
  </BlurOverlay>
{/if}

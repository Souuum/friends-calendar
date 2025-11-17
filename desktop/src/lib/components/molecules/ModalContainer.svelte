<script lang="ts">
  export let isOpen = false;
  export let size: 'sm' | 'md' | 'lg' | 'xl' | 'full' = 'md';
  export let position: 'center' | 'top' | 'bottom' = 'center';
  export let onClose: (() => void) | undefined = undefined;
  export let closeOnBackdrop = true;
  export let closeOnEscape = true;

  const sizeClasses = {
    sm: 'max-w-md',
    md: 'max-w-2xl',
    lg: 'max-w-4xl',
    xl: 'max-w-6xl',
    full: 'max-w-full mx-4'
  };

  const positionClasses = {
    center: 'items-center',
    top: 'items-start pt-20',
    bottom: 'items-end pb-20'
  };

  function handleBackdropClick(e: MouseEvent) {
    if (closeOnBackdrop && e.target === e.currentTarget && onClose) {
      onClose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (closeOnEscape && e.key === 'Escape' && isOpen && onClose) {
      onClose();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if isOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fixed inset-0 z-50 flex justify-center p-4 {positionClasses[position]}"
    on:click={handleBackdropClick}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div
      role="button"
      tabindex="0"
      class="bg-white rounded-lg shadow-xl w-full {sizeClasses[
        size
      ]} max-h-[90vh] overflow-hidden animate-in zoom-in-95 duration-200"
      on:click={(e) => e.stopPropagation()}
    >
      <slot />
    </div>
  </div>
{/if}

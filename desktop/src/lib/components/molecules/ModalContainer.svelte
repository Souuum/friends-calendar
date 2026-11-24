<script lang="ts">
  /** Whether the dialog is rendered. Nothing is in the DOM while `false`. */
  export let isOpen = false;
  /** Maximum width of the panel: `sm` (28rem), `md` (42rem), `lg` (56rem), `xl` (72rem) or `full`. */
  export let size: 'sm' | 'md' | 'lg' | 'xl' | 'full' = 'md';
  /** Where the panel sits vertically in the viewport: `center`, `top` or `bottom`. */
  export let position: 'center' | 'top' | 'bottom' = 'center';
  /** Called when a backdrop click or Escape asks to close. The caller sets `isOpen` to `false`; without it neither closes anything. */
  export let onClose: (() => void) | undefined = undefined;
  /** Whether a click outside the panel calls `onClose`. */
  export let closeOnBackdrop = true;
  /** Whether pressing Escape while open calls `onClose`. */
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
      class="bg-surface rounded-lg shadow-xl w-full {sizeClasses[
        size
      ]} max-h-[90vh] overflow-hidden anim-pop"
      on:click={(e) => e.stopPropagation()}
    >
      <slot />
    </div>
  </div>
{/if}

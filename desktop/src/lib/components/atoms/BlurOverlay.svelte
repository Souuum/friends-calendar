<script lang="ts">
  /** Whether the overlay is rendered at all. `false` renders nothing, slot included. */
  export let visible = false;
  /** How strongly the page behind is blurred. */
  export let blurAmount: 'sm' | 'md' | 'lg' = 'sm';
  /** How dark the tint over the page is: 30%, 50% or 70%. */
  export let opacity: 'light' | 'medium' | 'dark' = 'medium';

  const blurClasses = {
    sm: 'backdrop-blur-xs',
    md: 'backdrop-blur-sm',
    lg: 'backdrop-blur-md'
  };

  const opacityClasses = {
    light: 'bg-primary-hover/30',
    medium: 'bg-primary-hover/50',
    dark: 'bg-primary-hover/70'
  };
</script>

{#if visible}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fixed inset-0 z-40 anim-scrim {blurClasses[blurAmount]} {opacityClasses[opacity]}"
    on:click
    role="button"
    tabindex="-1"
    aria-label="Close overlay"
  >
    <slot />
  </div>
{/if}

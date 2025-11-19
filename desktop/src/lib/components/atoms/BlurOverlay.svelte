<script lang="ts">
  export let visible = false;
  export let blurAmount: 'sm' | 'md' | 'lg' = 'sm';
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
    class="fixed inset-0 z-40 transition-all duration-200 {blurClasses[blurAmount]} {opacityClasses[
      opacity
    ]}"
    class:animate-in={visible}
    class:fade-in={visible}
    on:click
    role="button"
    tabindex="-1"
    aria-label="Close overlay"
  >
    <slot />
  </div>
{/if}

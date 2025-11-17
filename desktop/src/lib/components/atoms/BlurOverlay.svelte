<script lang="ts">
  export let isVisible = false;
  export let onClick: (() => void) | undefined = undefined;
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

  function handleClick(e: MouseEvent) {
    if (onClick) {
      onClick();
    }
  }
</script>

{#if isVisible}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="fixed inset-0 z-40 transition-all duration-200 {blurClasses[blurAmount]} {opacityClasses[
      opacity
    ]}"
    class:animate-in={isVisible}
    class:fade-in={isVisible}
    on:click={handleClick}
    role="button"
    tabindex="-1"
    aria-label="Close overlay"
  >
    <slot />
  </div>
{/if}

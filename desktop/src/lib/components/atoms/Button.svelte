<script lang="ts">
  import type { ButtonType } from '$lib/types';

  /** Visual style. `primary` is the only one defined; any other value renders unstyled. */
  export let variant = 'primary';
  /** Native `type` attribute. Use `submit` inside a `<form>` so Enter submits it. */
  export let type: ButtonType = 'button';
</script>

<button
  {type}
  class={`px-4 py-2 rounded-lg font-medium transition
    ${variant === 'primary' ? 'bg-discord-blurple hover:bg-blue-600 text-white' : ''}
  `}
>
  <slot />
</button>

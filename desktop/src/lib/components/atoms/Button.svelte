<script lang="ts">
 import type { ButtonType } from '$lib/types';
  export let variant = "primary";
  export let type: ButtonType = "button";
</script>

<button
  type={type}
  class={`px-4 py-2 rounded-lg font-medium transition
    ${variant === "primary" ? "bg-discord-blurple hover:bg-blue-600 text-white" : ""}
  `}
>
  <slot />
</button>
<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ChevronDown from '$lib/components/icons/ChevronDown.svelte';
  import ChevronUp from '$lib/components/icons/ChevronUp.svelte';

  /**
   * `ChevronDown` and `ChevronUp`: the open/closed indicator of a disclosure control - down
   * while it is closed, up while it is open. The profile menu trigger is the one place that
   * uses them. They are separate from `Icon.svelte`, which has no chevron-up/down glyphs.
   *
   * ```svelte
   * <button class="p-1 rounded-lg text-muted" on:click={() => (show = !show)}>
   *   {#if show}
   *     <ChevronUp />
   *   {:else}
   *     <ChevronDown />
   *   {/if}
   * </button>
   * ```
   *
   * - No props, no slot, no events - both files are a bare `<svg>`.
   * - Always 24x24px; the size is set in the component's own style and cannot be changed by
   *   the caller.
   * - Drawn with `stroke="currentColor"`, so the colour is the surrounding text colour.
   * - `aria-hidden`: the button around it needs its own accessible name.
   */
  const { Story } = defineMeta({
    title: 'Components/Icons/Chevrons',
    component: ChevronDown
  });
</script>

<!-- `ChevronDown` on its own: the "closed" indicator. -->
<Story name="Down" />

<!-- `ChevronUp` on its own: the "open" indicator. -->
<Story name="Up" asChild>
  <ChevronUp />
</Story>

<!-- Both chevrons inside a `text-primary` wrapper: the stroke follows the parent's text colour. -->
<Story name="Inherits text colour" asChild>
  <div class="flex gap-4 text-primary">
    <ChevronDown />
    <ChevronUp />
  </div>
</Story>

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ChevronDown from '$lib/components/icons/ChevronDown.svelte';
  import ChevronUp from '$lib/components/icons/ChevronUp.svelte';

  // The profile menu trigger's open/closed indicator. Separate from Icon.svelte,
  // which has no chevron-up/down glyphs.
  const { Story } = defineMeta({
    title: 'Components/Icons/Chevrons',
    component: ChevronDown
  });
</script>

<Story name="Down" />

<Story name="Up">
  <ChevronUp />
</Story>

<Story name="Inherits text colour">
  <div class="flex gap-4 text-primary">
    <ChevronDown />
    <ChevronUp />
  </div>
</Story>

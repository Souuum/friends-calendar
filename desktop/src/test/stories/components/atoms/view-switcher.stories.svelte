<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import type { ComponentProps } from 'svelte';
  import ViewSwitcher from '$lib/components/atoms/ViewSwitcher.svelte';
  import ViewSwitcherStory from './ViewSwitcherStory.svelte';

  /**
   * The segmented Day / Week / Month / List control of the calendar header.
   *
   * ```svelte
   * <ViewSwitcher {view} on:view-change={(e) => (view = e.detail)} />
   * ```
   *
   * Clicking a segment dispatches `view-change` with the chosen view as its
   * detail. It also assigns `view` itself, so `bind:view` works as well.
   * `view` is required and has no default.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/ViewSwitcher',
    component: ViewSwitcher,
    argTypes: {
      view: { control: 'inline-radio', options: ['day', 'week', 'month', 'list'] }
    },
    args: { view: 'month' }
  });
</script>

<!-- A story-only wrapper binds `view` and prints it with the last `view-change` detail. -->
{#snippet bound(args: ComponentProps<typeof ViewSwitcher>)}
  <ViewSwitcherStory {...args} />
{/snippet}

<!-- `view="month"`: click a segment to see the bound value and the event detail change. -->
<Story name="Default" template={bound} />

<!-- `view="day"` highlights the Day segment. -->
<Story name="Day" args={{ view: 'day' }} template={bound} />

<!-- `view="week"` highlights the Week segment. -->
<Story name="Week" args={{ view: 'week' }} template={bound} />

<!-- `view="list"` highlights the List segment. -->
<Story name="List" args={{ view: 'list' }} template={bound} />

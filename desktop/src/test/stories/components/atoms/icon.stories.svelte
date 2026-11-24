<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Icon, { type IconName } from '$lib/components/atoms/Icon.svelte';

  const NAMES: IconName[] = [
    'calendar',
    'friends',
    'announcements',
    'notifications',
    'settings',
    'bot',
    'time',
    'location',
    'price',
    'link',
    'accept',
    'decline',
    'replies',
    'reactions',
    'pinned',
    'light-mode',
    'dark-mode',
    'back',
    'chevron-right',
    'search'
  ];

  /**
   * An outline icon from the app's own set: a 24x24 SVG with 2px strokes that
   * takes the colour of the surrounding text.
   *
   * ```svelte
   * <span class="flex items-center gap-1 text-muted">
   *   <Icon name="location" size={16} />
   *   {event.location}
   * </span>
   * ```
   *
   * Names describe the meaning, not the shape (`price`, not a currency sign).
   * The icon is hidden from screen readers unless `label` is set, because it
   * nearly always sits next to its own text. It never shrinks in a flex row.
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/Icon',
    component: Icon,
    argTypes: {
      name: { control: 'select', options: NAMES },
      size: { control: 'number' }
    },
    args: { name: 'calendar', size: 24 }
  });
</script>

<!-- One icon chosen by `name`, at `size={24}`. -->
<Story name="Default" />

<!-- `label` makes the icon an image with an accessible name instead of hiding it. -->
<Story name="Labelled" args={{ name: 'notifications', label: 'Notifications' }} />

<!-- `size={48}`: the width and height in pixels. The strokes scale with it. -->
<Story name="Large" args={{ size: 48 }} />

<!-- `class` is added to the SVG; a text colour class recolours the strokes. -->
<Story name="Coloured through class" args={{ name: 'accept', class: 'text-primary' }} />

<!-- Every value `name` accepts, each beside its name. -->
<Story name="All icons" asChild>
  <div class="grid grid-cols-4 gap-4 text-gray-700">
    {#each NAMES as name (name)}
      <div class="flex items-center gap-2 text-sm">
        <Icon {name} size={20} />
        <span class="font-mono text-xs">{name}</span>
      </div>
    {/each}
  </div>
</Story>

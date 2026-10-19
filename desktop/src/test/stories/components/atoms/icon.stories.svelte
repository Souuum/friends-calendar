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

<Story name="Default" />

<Story name="Labelled" args={{ name: 'notifications', label: 'Notifications' }} />

<Story name="All icons">
  <div class="grid grid-cols-4 gap-4 text-gray-700">
    {#each NAMES as name (name)}
      <div class="flex items-center gap-2 text-sm">
        <Icon {name} size={20} />
        <span class="font-mono text-xs">{name}</span>
      </div>
    {/each}
  </div>
</Story>

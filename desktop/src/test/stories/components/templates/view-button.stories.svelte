<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ViewButton from '$lib/components/templates/ViewButton.svelte';

  /**
   * One row of the desktop sidebar: a 6px dot, the label and an optional count. `Frame` renders
   * one per navigation entry; use it for any list of routes where exactly one is the current
   * page.
   *
   * ```svelte
   * {#each navItems as item}
   *   <ViewButton
   *     {item}
   *     current={$page.url.pathname === item.view}
   *     on:click={() => goto(item.view)}
   *   />
   * {/each}
   * ```
   *
   * - Forwards `click` and does nothing else with it: navigating is the caller's job, which is
   *   why `item.view` is carried along but never read here.
   * - Fills the width of its container (`w-full`), so it needs a parent that sets the width -
   *   the rail is 216px.
   * - No slot. Dots, not icons, on purpose: icons belong to `BottomTabBar`.
   */
  const { Story } = defineMeta({
    title: 'Components/Templates/ViewButton',
    component: ViewButton,
    args: { item: { label: 'Friends', view: '/friends' } },
    argTypes: { current: { control: 'boolean' } }
  });
</script>

<!-- `item` with a label only, not current: plain text and a grey dot. -->
<Story name="Default" />

<!-- `current`: tint pill, accent dot, bold label and `aria-current="page"`. -->
<Story name="Current" args={{ current: true }} />

<!-- `item.badge`: a count on the right, as on the Notifications row. -->
<Story
  name="With badge"
  args={{ item: { label: 'Notifications', view: '/notifications', badge: 12 } }}
/>

<!-- `current` and `item.badge` together: the count takes the accent colour too. -->
<Story
  name="Current with badge"
  args={{ current: true, item: { label: 'Notifications', view: '/notifications', badge: 12 } }}
/>

<!-- `item.badge` of 0 is not rendered, so an empty count never shows as "0". -->
<Story
  name="Badge of zero"
  args={{ item: { label: 'Notifications', view: '/notifications', badge: 0 } }}
/>

<!-- Several rows in a 216px rail, one of them `current` - how `Frame` lays them out. -->
<Story name="In a rail" asChild>
  <nav class="flex w-[216px] flex-col gap-0.5 bg-surface p-2.5">
    <ViewButton item={{ label: 'Calendar', view: '/' }} />
    <ViewButton item={{ label: 'Friends', view: '/friends' }} current />
    <ViewButton item={{ label: 'Notifications', view: '/notifications', badge: 3 }} />
    <ViewButton item={{ label: 'Settings', view: '/settings' }} />
  </nav>
</Story>

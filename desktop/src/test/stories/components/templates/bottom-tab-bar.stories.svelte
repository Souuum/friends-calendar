<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import BottomTabBar from '$lib/components/templates/BottomTabBar.svelte';
  import { unreadNotificationCount } from '$lib/stores';

  const onRoute = (pathname: string) => ({
    sveltekit_experimental: { stores: { page: { url: new URL(pathname, 'http://localhost') } } }
  });

  /**
   * The mobile navigation: five icon tabs - Calendar, Friends, Hub, Alerts, Me - pinned to the
   * bottom of the viewport below `md:` (768px). `Frame` mounts it once for every page, so a
   * route never renders it itself.
   *
   * ```svelte
   * <main class="pb-24 md:pb-[60px]">
   *   <slot />
   * </main>
   * <BottomTabBar />
   * ```
   *
   * - No props, no slot, no events. Each tab calls `goto()` on its own route.
   * - Reads `$page.url.pathname` to mark the current tab. The match is exact, so a nested route
   *   such as `/friends/u1` highlights no tab.
   * - Reads the `unreadNotificationCount` store for the Alerts badge; a count of 0 shows no
   *   badge. It does not fetch the count - `Header` does.
   * - Renders `position: fixed` and is `md:hidden`: the page needs bottom padding so content
   *   does not sit under it, and from 768px up it renders nothing. These stories open on a
   *   mobile viewport; in a wider frame only a hint is shown.
   * - Not the same list as the desktop rail: "Discord server" and "Add friends" have no tab.
   */
  const { Story } = defineMeta({
    title: 'Components/Templates/BottomTabBar',
    component: BottomTabBar,
    globals: { viewport: { value: 'mobile1', isRotated: false } },
    parameters: {
      layout: 'fullscreen',
      // Fixed to the viewport, and every story sets the shared unread-count store, so on the
      // docs page each story needs a frame of its own.
      docs: { story: { inline: false, height: '120px' } },
      ...onRoute('/')
    },
    beforeEach: () => {
      unreadNotificationCount.set(3);
      return () => unreadNotificationCount.set(0);
    }
  });
</script>

<!-- The bar is `md:hidden`, so a frame of 768px or wider would otherwise be blank. -->
{#snippet bar()}
  <p class="hidden p-4 text-sm text-muted md:block">
    BottomTabBar is hidden from 768px up - narrow the frame to see it.
  </p>
  <BottomTabBar />
{/snippet}

<!-- On `/`: the Calendar tab is current, and Alerts carries the unread count. -->
<Story name="Calendar tab" template={bar} />

<!-- On `/friends`: the Friends tab is current. -->
<Story name="Friends tab" parameters={onRoute('/friends')} template={bar} />

<!-- On `/notifications`: the Alerts tab is current, with its badge still shown. -->
<Story name="Alerts tab" parameters={onRoute('/notifications')} template={bar} />

<!-- On `/announcements`: the Hub tab is current. -->
<Story name="Hub tab" parameters={onRoute('/announcements')} template={bar} />

<!-- On `/settings`: the Me tab is current. -->
<Story name="Me tab" parameters={onRoute('/settings')} template={bar} />

<!-- `unreadNotificationCount` at 0: the Alerts tab has no badge. -->
<Story
  name="No unread alerts"
  beforeEach={() => {
    unreadNotificationCount.set(0);
  }}
  template={bar}
/>

<!-- On `/friends/u1`: the match is exact, so no tab is current on a nested route. -->
<Story name="On a nested route" parameters={onRoute('/friends/u1')} template={bar} />

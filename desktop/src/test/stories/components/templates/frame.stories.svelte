<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Frame from '$lib/components/templates/Frame.svelte';
  import { unreadNotificationCount, user } from '$lib/stores';
  import { resetAdminStatus } from '$lib/admin';
  import { ME } from '../../fixtures';
  import { mockApi } from '../../mockApi';

  const onRoute = (pathname: string) => ({
    sveltekit_experimental: { stores: { page: { url: new URL(pathname, 'http://localhost') } } }
  });

  /**
   * The app shell every signed-in page renders inside: the 216px navigation rail and the header
   * from `md:` (768px) up, `BottomTabBar` below it, and the page itself in the default slot.
   *
   * ```svelte
   * <Frame>
   *   <CalendarView />
   * </Frame>
   * ```
   *
   * - No props and no events. The default slot is the page content, centred in a 1080px
   *   column that already has its padding - a page adds neither.
   * - Reads `$page.url.pathname` for the current rail row (exact match, so `/friends/u1`
   *   highlights none) and for the header title, which falls back to the section name on
   *   nested routes.
   * - Reads the `user` store (avatar and name at the foot of the rail and in the header), the
   *   `unreadNotificationCount` store (Notifications badge) and the `isAdmin` store (the
   *   "Discord server" row is only there for admins).
   * - On mount it calls `loadAdminStatus()`, which asks `api.getAdminStatus()` once per
   *   session, and its `Header` calls `api.getUnreadNotificationCount()`.
   * - Rail rows navigate with `goto()`. Logging out from the header clears the token and
   *   reloads the page.
   * - From `md:` up the shell is exactly viewport height and only the content column scrolls.
   */
  const { Story } = defineMeta({
    title: 'Components/Templates/Frame',
    component: Frame,
    parameters: {
      layout: 'fullscreen',
      // Every story sets shared stores and stubs `api`, and the shell sizes itself to the
      // viewport, so on the docs page each story needs a frame of its own.
      docs: { story: { inline: false, height: '700px' } },
      ...onRoute('/')
    },
    beforeEach: signedIn({ admin: true })
  });

  /**
   * The admin answer is cached for the session, so each story clears it -
   * otherwise whichever story ran first would decide the sidebar for all.
   */
  function signedIn({ admin }: { admin: boolean }) {
    return () => {
      user.set(ME);
      resetAdminStatus();
      const restore = mockApi({
        getUnreadNotificationCount: async () => 5,
        getAdminStatus: async () => admin
      });
      return () => {
        restore();
        resetAdminStatus();
        // `Header` wrote the stubbed count into the shared store.
        unreadNotificationCount.set(0);
        user.set(null);
      };
    };
  }
</script>

<!-- The slot is filled through a template: children of `<Story>` would not reach `<slot />`. -->
{#snippet withPage()}
  <Frame>
    <div class="space-y-3">
      <h1 class="text-xl font-semibold">Page content</h1>
      <p class="text-sm text-muted">Each route renders inside this shell.</p>
    </div>
  </Frame>
{/snippet}

<!-- On `/` as an admin: Calendar is the current row, with every rail entry and 5 unread notifications. -->
<Story name="Calendar route" template={withPage} />

<!-- `isAdmin` false: no "Discord server" row - that is the bot's configuration, admins only. -->
<Story name="As a non-admin" beforeEach={signedIn({ admin: false })} template={withPage} />

<!-- On `/settings`: the current row and the header title follow the route. -->
<Story name="Settings route" parameters={onRoute('/settings')} template={withPage} />

<!-- On `/friends/u1`: no row is current on a nested route, and the header falls back to "Friends". -->
<Story name="Friend profile route" parameters={onRoute('/friends/u1')} template={withPage} />

<!-- Below 768px the rail is gone and `BottomTabBar` takes over; the canvas opens on a mobile viewport. -->
<Story
  name="Mobile (bottom tab bar)"
  globals={{ viewport: { value: 'mobile1', isRotated: false } }}
  template={withPage}
/>

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Header from '$lib/components/organisms/Header.svelte';
  import { ME } from '../../fixtures';
  import { mockApi } from '../../mockApi';
  import { writable } from 'svelte/store';
  import mimo from '../../assets/mimo.png';

  /**
   * The top bar of every screen: the screen's title on the left, then the
   * theme toggle, the notifications bell and the profile menu. `Frame` renders
   * it; a page should not need to mount it directly.
   *
   * ```svelte
   * <Header {user} {avatarUrl} title="Calendar" />
   * ```
   *
   * - `user` is the **store** from `$lib/stores`, not a `User` object - the
   *   component subscribes to it. A plain object throws.
   * - On mount it fetches `api.getUnreadNotificationCount()` into the shared
   *   `unreadNotificationCount` store, which drives the dot on the bell.
   * - Dispatches no events. The bell and "Settings" navigate with `goto`, and
   *   "Log out" clears the token and reloads the page.
   * - Reads the `resolvedTheme` store: the toggle shows the mode it switches to.
   */
  const { Story } = defineMeta({
    title: 'Components/Organisms/Header',
    component: Header,
    parameters: {
      layout: 'fullscreen',
      docs: { story: { inline: false, height: '260px' } }
    },
    // `user` is the store itself (Frame passes `$lib/stores`' `user`), not a User.
    args: { user: writable(ME), avatarUrl: mimo, title: 'Calendar' },
    beforeEach: () => mockApi({ getUnreadNotificationCount: async () => 3 })
  });
</script>

<!-- `user`, `avatarUrl` and `title` set, with unread notifications: the bell carries a dot. -->
<Story name="With unread notifications" />

<!-- The same props with an unread count of 0: no dot on the bell. -->
<Story
  name="Nothing unread"
  beforeEach={() => mockApi({ getUnreadNotificationCount: async () => 0 })}
/>

<!-- A different `title`: the bar names whichever screen is showing. -->
<Story name="Another screen" args={{ title: 'Friends' }} />

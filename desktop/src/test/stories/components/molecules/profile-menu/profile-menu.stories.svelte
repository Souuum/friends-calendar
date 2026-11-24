<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ProfileMenu from '$lib/components/molecules/ProfileMenu/ProfileMenu.svelte';
  import ProfileMenuStory from './ProfileMenuStory.svelte';

  /**
   * The two entries of the profile dropdown: "Settings" and "Logout". Use it as
   * the content of the menu that `ProfileMenuTrigger` opens.
   *
   * ```svelte
   * {#if show}
   *   <div class="absolute right-0 rounded-lg border border-line bg-surface p-1 shadow-lg">
   *     <ProfileMenu on:settings={goToSettings} on:logout={handleLogout} />
   *   </div>
   * {/if}
   * ```
   *
   * - No props. It dispatches `settings` and `logout`, both without a `detail`,
   *   and does nothing else: navigating and signing out are the caller's job.
   * - Just the two buttons: the caller supplies the surrounding panel, its
   *   position and when it is shown.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/ProfileMenu/ProfileMenu',
    component: ProfileMenu
  });
</script>

<!-- Both entries. A story-only wrapper listens for `settings` and `logout` and shows an alert naming the event. -->
<Story name="Default" asChild>
  <ProfileMenuStory />
</Story>

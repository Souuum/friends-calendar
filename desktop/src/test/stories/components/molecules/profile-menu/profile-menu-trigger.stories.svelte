<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import ProfileMenuTrigger from '$lib/components/molecules/ProfileMenu/ProfileMenuTrigger.svelte';
  import mimo from '../../../assets/mimo.png';

  /**
   * The signed-in user in the header: avatar, name and a chevron. It is the
   * handle that opens the profile menu. The menu itself is `ProfileMenu`, which
   * the caller renders.
   *
   * ```svelte
   * <ProfileMenuTrigger {username} avatar={avatarUrl} bind:show on:click={toggleMenu} />
   * {#if show}
   *   <ProfileMenu on:settings={goToSettings} on:logout={handleLogout} />
   * {/if}
   * ```
   *
   * - Forwards `click` from the avatar only. That click does not change `show`.
   * - The chevron button flips `show` itself and dispatches nothing, so the
   *   caller only sees that change through `bind:show`.
   * - The name is hidden below the `sm` breakpoint.
   */
  const { Story } = defineMeta({
    title: 'Components/Molecules/ProfileMenu/ProfileMenuTrigger',
    component: ProfileMenuTrigger
  });
</script>

<!-- Menu closed (`show` is `false`): the chevron points down. -->
<Story name="Default" args={{ avatar: mimo, username: 'Jon Doe' }} />

<!-- `show={true}`: the chevron points up. -->
<Story name="Menu open" args={{ avatar: mimo, username: 'Jon Doe', show: true }} />

<!-- A long `username` is truncated at 180px. -->
<Story
  name="Long username"
  args={{ avatar: mimo, username: 'a-discord-handle-with-no-sensible-length-limit' }}
/>

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Avatar from '$lib/components/atoms/Avatar.svelte';
  import mimo from '../../assets/mimo.png';

  /**
   * A round picture inside a button. It shows the signed-in user on the profile
   * menu trigger, the author of an announcement and the icon of the linked server.
   *
   * ```svelte
   * <Avatar src={avatarUrl} size={32} class="cursor-pointer" on:click={toggleMenu} />
   * ```
   *
   * It forwards `on:click`. `class` lands on the button, not on the image.
   * There is no fallback picture for an empty `src`, and the alt text is
   * always "User avatar".
   */
  const { Story } = defineMeta({
    title: 'Components/Atoms/Avatar',
    component: Avatar
  });
</script>

<!-- Only `src` set: the default `size` of 48px. -->
<Story name="Default" args={{ src: mimo }} />

<!-- `size={128}` sets both the width and the height of the picture, in pixels. -->
<Story name="Default x128" args={{ src: mimo, size: 128 }} />

<!-- `size={32}` as in the header, with `class` adding a pointer cursor to the button. -->
<Story name="Small with a class" args={{ src: mimo, size: 32, class: 'cursor-pointer' }} />

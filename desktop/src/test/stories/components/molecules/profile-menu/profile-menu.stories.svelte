<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
    import ProfileMenu from './ProfileMenuStory.svelte';
  
    const { Story } = defineMeta({
      title: 'Components/Molecules/ProfileMenu/ProfileMenu',
      component: ProfileMenu
    });
  </script>
  
  <Story name="Default" />
  
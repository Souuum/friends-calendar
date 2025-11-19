<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
    import ProfileMenuTrigger from '$lib/components/molecules/ProfileMenu/ProfileMenuTrigger.svelte';
    import mimo from "../../../assets/mimo.png";
    
    const { Story } = defineMeta({
      title: 'Components/Molecules/ProfileMenu/ProfileMenuTrigger',
      component: ProfileMenuTrigger
    });
  </script>
  
  <Story name="Default" args={{avatar: mimo, username: "Jon Doe"}}/>
  
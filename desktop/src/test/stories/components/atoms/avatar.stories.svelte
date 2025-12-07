<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
    import Avatar from '$lib/components/atoms/Avatar.svelte';
    import mimo from "../../assets/mimo.png";
  
    const { Story } = defineMeta({
      title: 'Components/Atoms/Avatar',
      component: Avatar
    });
</script>
  
<Story name="Default" args={{src: mimo}}/>
  

  
<Story name="Default x128" args={{src: mimo, size: 128}}/>

<script module>
    import { defineMeta } from '@storybook/addon-svelte-csf';
  import ViewSwitcherStory from './ViewSwitcherStory.svelte';
  
    const { Story } = defineMeta({
      title: 'Components/Atoms/ViewSwitcher',
      component: ViewSwitcherStory
    });
  </script>
  
<Story name="Default" />
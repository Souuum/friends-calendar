<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import BottomTabBar from '$lib/components/templates/BottomTabBar.svelte';
  import { unreadNotificationCount } from '$lib/stores';

  const onRoute = (pathname: string) => ({
    sveltekit_experimental: { stores: { page: { url: new URL(pathname, 'http://localhost') } } }
  });

  const { Story } = defineMeta({
    title: 'Components/Templates/BottomTabBar',
    component: BottomTabBar,
    parameters: { layout: 'fullscreen', ...onRoute('/') },
    beforeEach: () => {
      unreadNotificationCount.set(3);
      return () => unreadNotificationCount.set(0);
    }
  });
</script>

<!-- `md:hidden` - narrow the canvas below 768px to see it. -->
<Story name="Calendar tab" />
<Story name="Friends tab" parameters={onRoute('/friends')} />
<Story name="Alerts tab" parameters={onRoute('/notifications')} />

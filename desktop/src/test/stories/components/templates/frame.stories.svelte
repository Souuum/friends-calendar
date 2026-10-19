<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Frame from '$lib/components/templates/Frame.svelte';
  import { user } from '$lib/stores';
  import { ME } from '../../fixtures';
  import { mockApi } from '../../mockApi';

  const onRoute = (pathname: string) => ({
    sveltekit_experimental: { stores: { page: { url: new URL(pathname, 'http://localhost') } } }
  });

  const { Story } = defineMeta({
    title: 'Components/Templates/Frame',
    component: Frame,
    parameters: { layout: 'fullscreen', ...onRoute('/') },
    beforeEach: () => {
      user.set(ME);
      const restore = mockApi({ getUnreadNotificationCount: async () => 5 });
      return () => {
        restore();
        user.set(null);
      };
    }
  });
</script>

{#snippet withPage()}
  <Frame>
    <div class="space-y-3">
      <h1 class="text-xl font-semibold">Page content</h1>
      <p class="text-sm text-muted">Each route renders inside this shell.</p>
    </div>
  </Frame>
{/snippet}

<Story name="Calendar route" template={withPage} />
<Story name="Settings route" parameters={onRoute('/settings')} template={withPage} />

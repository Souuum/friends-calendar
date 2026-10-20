<script module lang="ts">
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Frame from '$lib/components/templates/Frame.svelte';
  import { user } from '$lib/stores';
  import { resetAdminStatus } from '$lib/admin';
  import { ME } from '../../fixtures';
  import { mockApi } from '../../mockApi';

  const onRoute = (pathname: string) => ({
    sveltekit_experimental: { stores: { page: { url: new URL(pathname, 'http://localhost') } } }
  });

  const { Story } = defineMeta({
    title: 'Components/Templates/Frame',
    component: Frame,
    parameters: { layout: 'fullscreen', ...onRoute('/') },
    beforeEach: signedIn({ admin: true })
  });

  /**
   * The admin answer is cached for the session, so each story clears it -
   * otherwise whichever story ran first would decide the sidebar for all.
   */
  function signedIn({ admin }: { admin: boolean }) {
    return () => {
      user.set(ME);
      resetAdminStatus();
      const restore = mockApi({
        getUnreadNotificationCount: async () => 5,
        getAdminStatus: async () => admin
      });
      return () => {
        restore();
        resetAdminStatus();
        user.set(null);
      };
    };
  }
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
<!-- No "Discord server" entry: that's the bot's configuration, admins only. -->
<Story name="As a non-admin" beforeEach={signedIn({ admin: false })} template={withPage} />
<Story name="Settings route" parameters={onRoute('/settings')} template={withPage} />

<script module>
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import Header from '$lib/components/organisms/Header.svelte';
  import { ME } from '../../fixtures';
  import { mockApi } from '../../mockApi';
  import { writable } from 'svelte/store';
  import mimo from '../../assets/mimo.png';

  const { Story } = defineMeta({
    title: 'Components/Organisms/Header',
    component: Header,
    parameters: { layout: 'fullscreen' },
    // `user` is the store itself (Frame passes `$lib/stores`' `user`), not a User.
    args: { user: writable(ME), avatarUrl: mimo, title: 'Calendar' },
    beforeEach: () => mockApi({ getUnreadNotificationCount: async () => 3 })
  });
</script>

<Story name="With unread notifications" />
<Story
  name="Nothing unread"
  beforeEach={() => mockApi({ getUnreadNotificationCount: async () => 0 })}
/>
